

//
//  Copyright 2022 by Rodrigo Kellermann Ferreira <rkferreira@gmail.com>
//
/*
Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/


use std::str;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_ecr::{Error, Region, PKG_VERSION};
use structopt::StructOpt;
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::api::core::v1::Namespace;
use kube::{client::ConfigExt, Api, Client, Config, ResourceExt};
use kube::core::params::PostParams;


#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(short, long)]
    region: Option<String>,
    #[structopt(default_value = "aws-ecr-auth", long)]
    secretname: String,
}

async fn create_json_secret (auth_token: &str, ecr_endpoint: &str) -> String {
    let ep = &ecr_endpoint[8..];
    let mut b64_auth_token: String = str::from_utf8(&base64::decode(&auth_token).unwrap()).unwrap().to_string();
    b64_auth_token = b64_auth_token.split(":").nth(1).unwrap().to_string();
    let concat = format!("{{\"auths\":{{\"{}\":{{\"username\":\"AWS\",\"password\":\"{}\",\"auth\":\"{}\"}}}}}}", ep, b64_auth_token, auth_token);
    // println!("{}", concat);

    let secret_data: String = base64::encode(&concat);
    // println!(".dockerconfigjson: {}", secret_data);
    secret_data
}

async fn k8s(json_secret: &str, secret_name: &str) -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=trace");
    tracing_subscriber::fmt::init();

    let config = Config::infer().await?;
    let https = config.openssl_https_connector()?;
    let service = tower::ServiceBuilder::new()
        .layer(config.base_uri_layer())
        .option_layer(config.auth_layer()?)
        .service(hyper::Client::builder().build(https));
    let client = Client::new(service.clone(), config.default_namespace);

    let ns: Api<Namespace> = Api::all(client.clone());
    for n in ns.list(&Default::default()).await? {
        println!("namespace: {}", n.name());
        let secrets: Api<Secret> = Api::namespaced(client.clone(), &n.name());
        let r = secrets.get(&secret_name).await;
        if r.is_ok() {
            println!("{}: secret exists on namespace", n.name());
            let r = secrets.get(&secret_name).await?;
            let ecr_secret: Secret = serde_json::from_value(serde_json::json!({
                "apiVersion": "v1",
                "kind": "Secret",
                "metadata": {
                    "name": &secret_name,
                    "namespace": &n.name(),
                    "resourceVersion": r.resource_version(),
                },
                "type": "kubernetes.io/dockerconfigjson",
                "data": {
                    ".dockerconfigjson": &json_secret
                }
            }))?;
            secrets.replace(&secret_name, &PostParams::default(), &ecr_secret).await?;
            println!("{}: secret updated on namespace", n.name());
        } else {
            println!("{}: secret does not exist on namespace", n.name());
            let ecr_secret: Secret = serde_json::from_value(serde_json::json!({
                "apiVersion": "v1",
                "kind": "Secret",
                "metadata": {
                    "name": &secret_name,
                    "namespace": &n.name(),
                },
                "type": "kubernetes.io/dockerconfigjson",
                "data": {
                    ".dockerconfigjson": &json_secret
                }
            }))?;
            //println!("{}", serde_json::to_string_pretty(&ecr_secret).unwrap() );

            secrets.create(&PostParams::default(), &ecr_secret).await?;
            println!("{}: secret created on namespace", n.name());
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let Opt {
        region,
        secretname,
    } = Opt::from_args();

    let mut auth_token: String = String::from("None");
    let mut ecr_endpoint: String = String::from("None");
    let secret_name: &str = "aws-ecr-auth";

    println!("Welcome! Starting...");

    let region_provider = RegionProviderChain::first_try(region.map(Region::new))
        .or_default_provider()
        .or_else(Region::new("us-east-1"));


    let shared_config = aws_config::from_env().region(region_provider).load().await;
    let client = aws_sdk_ecr::Client::new(&shared_config);

    let auth_token_list = (client.get_authorization_token().send().await?).authorization_data.unwrap();
    // println!("{:?}", auth_token_list);
 
    if auth_token_list[0].authorization_token.is_some() && auth_token_list[0].proxy_endpoint.is_some() {
        let c = auth_token_list[0].clone();
        auth_token = (&c.authorization_token.unwrap()).to_string();
        ecr_endpoint = (&c.proxy_endpoint.unwrap()).to_string();
        // println!("{}", auth_token);
        // println!("{}", ecr_endpoint);
    }
    // println!("{}", auth_token);

    let json_secret: String = create_json_secret(&auth_token, &ecr_endpoint).await;
    // println!("{}", json_secret);

    k8s(&json_secret, &secret_name).await;
    Ok(())
}
