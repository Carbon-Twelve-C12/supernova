# Supernova DevOps Setup Guide

This guide outlines all necessary steps to set up DockerHub and Kubernetes integration for the Supernova blockchain project. Following these steps will ensure all GitHub Actions workflows pass successfully.

## Table of Contents
1. [Docker Hub Setup](#docker-hub-setup)
2. [GitHub Secrets Configuration](#github-secrets-configuration)
3. [Kubernetes Cluster Setup](#kubernetes-cluster-setup)
4. [Kubernetes Configuration for GitHub Actions](#kubernetes-configuration-for-github-actions)
5. [Testing the Workflows](#testing-the-workflows)
6. [Troubleshooting](#troubleshooting)

## Docker Hub Setup

### 1. Create a Docker Hub Account (if you don't have one)
- Go to [Docker Hub](https://hub.docker.com/) and sign up for an account
- Verify your email address

### 2. Create a Docker Hub Repository
- Login to Docker Hub
- Click the "Create Repository" button
- Set the repository name to exactly `supernova` (this will create `mjohnson518/supernova`)
- Set visibility to "Public" (or "Private" if required)
- Add a short description: "Supernova Blockchain Node - A production-grade PoW blockchain implementation in Rust"
- Add a full description or leave it blank (will be updated by our workflow)
- Click "Create"

### 3. Create a Docker Hub Access Token
- In Docker Hub, click on your username in the top-right corner
- Select "Account Settings"
- Navigate to the "Security" tab
- Click "New Access Token"
- Give it a descriptive name (e.g., "Supernova GitHub Actions")
- Set appropriate permissions (Read & Write access to repositories)
- Click "Generate"
- **IMPORTANT**: Copy the generated token and store it securely! You won't be able to see it again.

## GitHub Secrets Configuration

### 1. Add Docker Hub Credentials to GitHub Secrets
- Go to the [Supernova GitHub repository](https://github.com/mjohnson518/supernova)
- Navigate to "Settings" → "Secrets and variables" → "Actions"
- Add the following repository secrets:
  - Name: `DOCKERHUB_USERNAME`
    - Value: Your Docker Hub username
  - Name: `DOCKERHUB_TOKEN`
    - Value: The access token generated in previous step
  - Name: `DOCKERHUB_PASSWORD`
    - Value: Your Docker Hub password (needed for updating repository description)

## Kubernetes Cluster Setup

### 1. Choose a Kubernetes Provider
Choose one of the following options:
- **Local Development**: [Minikube](https://minikube.sigs.k8s.io/) or [kind](https://kind.sigs.k8s.io/)
- **Cloud Providers**:
  - [Google Kubernetes Engine (GKE)](https://cloud.google.com/kubernetes-engine)
  - [Amazon Elastic Kubernetes Service (EKS)](https://aws.amazon.com/eks/)
  - [Azure Kubernetes Service (AKS)](https://azure.microsoft.com/en-us/products/kubernetes-service)
  - [DigitalOcean Kubernetes](https://www.digitalocean.com/products/kubernetes)

### 2. Set Up Your Kubernetes Cluster
Follow the provider-specific instructions to create a cluster. Here's an example for Minikube:

```bash
# Install Minikube (if you haven't already)
# macOS
brew install minikube

# Start a Minikube cluster
minikube start --driver=docker --cpus=4 --memory=8g

# Verify the cluster is running
kubectl get nodes
```

### 3. Deploy Basic Infrastructure
Create the Supernova namespace and required resources:

```bash
# Create the namespace
kubectl create namespace supernova

# Apply initial configurations
kubectl apply -f kubernetes/supernova-deployment.yaml
```

### 4. Create Kubernetes Service Account for GitHub Actions
Create a service account with appropriate permissions:

```bash
# Create a service account
kubectl create serviceaccount github-actions -n supernova

# Grant necessary permissions (cluster-admin for simplicity, restrict in production)
kubectl create clusterrolebinding github-actions-admin \
  --clusterrole=cluster-admin \
  --serviceaccount=supernova:github-actions
```

## Kubernetes Configuration for GitHub Actions

### 1. Generate kubeconfig for GitHub Actions
Create a dedicated kubeconfig file that GitHub Actions will use:

```bash
# Get the token for the service account
SECRET_NAME=$(kubectl get serviceaccount github-actions -n supernova -o jsonpath='{.secrets[0].name}')
TOKEN=$(kubectl get secret $SECRET_NAME -n supernova -o jsonpath='{.data.token}' | base64 --decode)

# Get cluster info
CLUSTER_NAME=$(kubectl config current-context)
SERVER_URL=$(kubectl config view --minify -o jsonpath='{.clusters[0].cluster.server}')

# Create a basic kubeconfig
cat > github-actions-kubeconfig.yaml << EOF
apiVersion: v1
kind: Config
clusters:
- name: ${CLUSTER_NAME}
  cluster:
    server: ${SERVER_URL}
    certificate-authority-data: $(kubectl config view --flatten --minify -o jsonpath='{.clusters[0].cluster.certificate-authority-data}')
contexts:
- name: github-actions-context
  context:
    cluster: ${CLUSTER_NAME}
    user: github-actions
current-context: github-actions-context
users:
- name: github-actions
  user:
    token: ${TOKEN}
EOF
```

### 2. Add Kubernetes Config to GitHub Secrets

- Base64 encode the kubeconfig file:
  ```bash
  # Linux/macOS
  cat github-actions-kubeconfig.yaml | base64 -w 0
  
  # MacOS alternative if -w is not available
  cat github-actions-kubeconfig.yaml | base64
  ```

- Copy the entire base64-encoded output
- Go to the [Supernova GitHub repository settings](https://github.com/mjohnson518/supernova/settings/secrets/actions)
- Add two new repository secrets:
  - Name: `KUBE_CONFIG`
    - Value: The base64-encoded kubeconfig
  - Name: `KUBE_CONTEXT`
    - Value: `github-actions-context` (match the context name from the kubeconfig)

## Testing the Workflows

### 1. Test Docker Image Workflow First
- Go to the [Actions tab](https://github.com/mjohnson518/supernova/actions) in the repository
- Find the "Docker Image CI" workflow
- Click "Run workflow" and select the main branch
- Monitor the workflow execution and check for any errors

### 2. Test Deploy Workflow (Docker Only)
- After the Docker Image workflow succeeds:
- Find the "Deploy to Docker Hub and Kubernetes" workflow
- Click "Run workflow"
- Ensure "Skip Kubernetes deployment" is set to `true`
- Run the workflow and verify it completes successfully

### 3. Test Complete Deployment Workflow
- Once Docker-only deployment works:
- Run the "Deploy to Docker Hub and Kubernetes" workflow again
- Set "Skip Kubernetes deployment" to `false`
- Select the target environment (staging or production)
- Monitor the workflow for any errors

## Troubleshooting

### Docker Hub Issues

1. **Authentication Failures**:
   - Verify that `DOCKERHUB_USERNAME` and `DOCKERHUB_TOKEN` are correctly set in GitHub Secrets
   - Ensure the token has not expired (Docker Hub tokens expire after 30 days by default)
   - Check that the token has appropriate permissions (Read & Write)

2. **Push Failures**:
   - Verify that the repository exists on Docker Hub
   - Check that the repository name matches exactly (`mjohnson518/supernova`)
   - Ensure there are no issues with Docker Hub service status

3. **Build Failures**:
   - Check that the Dockerfile path is correct (`./docker/Dockerfile`)
   - Verify that all dependencies specified in the Dockerfile are available
   - Look for specific error messages in the build logs

### Kubernetes Issues

1. **Authentication Failures**:
   - Verify that `KUBE_CONFIG` is correctly base64-encoded
   - Ensure the service account token has not expired
   - Check that the context name in `KUBE_CONTEXT` matches the one in the kubeconfig

2. **Deployment Failures**:
   - Verify that the namespace exists (`kubectl get ns supernova`)
   - Check that the deployment files are valid (`kubectl apply --dry-run -f kubernetes/supernova-deployment.yaml`)
   - Ensure all required resources (PVs, ConfigMaps, etc.) are available
   - Check for specific error messages in the deployment logs

3. **Resource Constraints**:
   - Verify that your cluster has sufficient resources for the deployment
   - Check for resource quotas that might be limiting deployments

## Advanced Configuration

### Configuring Slack Notifications

If you want to receive Slack notifications for workflow status:

1. Create a Slack App and Webhook:
   - Go to [Slack API Apps](https://api.slack.com/apps)
   - Create New App → From Scratch
   - Add "Incoming Webhooks" feature
   - Create a new webhook URL for your workspace/channel

2. Add the webhook URL to GitHub Secrets:
   - Add a new repository secret:
     - Name: `SLACK_WEBHOOK_URL`
     - Value: Your Slack webhook URL

### Setting Up Separate Environments

For proper environment separation (staging vs. production):

1. Create GitHub Environments:
   - Go to repository Settings → Environments
   - Create two environments: "staging" and "production"
   - Add appropriate protection rules and secrets

2. Configure Kubernetes Namespaces:
   - Create separate namespaces:
     ```bash
     kubectl create namespace supernova-staging
     kubectl create namespace supernova-production
     ```

3. Update Deployment Files:
   - Create environment-specific deployment files
   - Modify the GitHub workflow to use the correct files based on the environment

## Regular Maintenance

### Docker Hub
- Periodically review and rotate access tokens
- Clean up old/unused images to save storage space
- Update repository descriptions and documentation as needed

### Kubernetes
- Keep the cluster up-to-date with security patches
- Monitor resource usage and scale as needed
- Review and rotate service account credentials
- Implement proper backup strategies for persistent volumes

## Security Best Practices

1. **Principle of Least Privilege**:
   - Use dedicated service accounts with minimal permissions
   - Avoid using cluster-admin role in production

2. **Secret Management**:
   - Regularly rotate all tokens and credentials
   - Consider using a secret management solution like HashiCorp Vault

3. **Image Security**:
   - Implement vulnerability scanning for container images
   - Use minimal base images to reduce attack surface
   - Never store sensitive information in Docker images

---

If you encounter any issues not covered in this guide, please reach out to the senior DevOps engineer or open an issue in the GitHub repository. 