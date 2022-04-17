# ecr-token-refresher

A container that keeps a fresh ECR token access available on kubernetes secrets.

Default behavior is to create **aws-ecr-auth** on all available namespaces.

It requires AWS access key with proper permission to ECR.

```
  AWS_ACCESS_KEY_ID
  AWS_SECRET_ACCESS_KEY
```

## steps

```
git clone <repo-url>

cd ecr-token-refresher

docker build -t ecr-token-refresher:latest .

docker tag ecr-token-refresher:latest rkferreira/ecr-token-refresher:latest

docker push rkferreira/ecr-token-refresher:latest
```

```
kubectl create ns ecr-token-refresher

helm install token ./helm -n ecr-token-refresher
```


