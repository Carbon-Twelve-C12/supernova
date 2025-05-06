# Supernova Helm Chart

A Helm chart for deploying the Supernova blockchain platform to Kubernetes.

## Introduction

This Helm chart deploys a complete Supernova blockchain infrastructure, including full nodes, mining nodes, monitoring tools, and backup systems. It is designed for production deployment with high availability, scalability, and security in mind.

## Prerequisites

- Kubernetes 1.20+
- Helm 3.1+
- PV provisioner support in the underlying infrastructure
- LoadBalancer support (or a suitable Ingress controller)

## Installing the Chart

To install the chart with the release name `supernova`:

```bash
# Add the Supernova Helm repository
helm repo add supernova https://charts.supernovanetwork.xyz

# Update the repository
helm repo update

# Install the chart with default values
helm install supernova supernova/supernova

# Install with a custom values file
helm install supernova supernova/supernova -f values.yaml
```

## Uninstalling the Chart

To uninstall/delete the `supernova` deployment:

```bash
helm uninstall supernova
```

## Configuration

The following table lists the configurable parameters of the Supernova chart and their default values.

| Parameter                                  | Description                                        | Default                               |
|--------------------------------------------|----------------------------------------------------|---------------------------------------|
| `global.environment`                      | Deployment environment                              | `production`                         |
| `global.imageRegistry`                    | Global Docker image registry                        | `""`                                 |
| `global.imagePullSecrets`                 | Global Docker registry secret names                 | `[]`                                 |
| `global.storageClass`                     | Global StorageClass for Persistent Volume Claims   | `"standard"`                         |
| `image.repository`                        | Image repository                                    | `supernova`                          |
| `image.tag`                               | Image tag                                          | `0.9.8`                              |
| `image.pullPolicy`                        | Image pull policy                                  | `IfNotPresent`                       |
| `fullNode.enabled`                        | Deploy full nodes                                  | `true`                               |
| `fullNode.replicaCount`                   | Number of full node replicas                       | `5`                                  |
| `fullNode.autoscaling.enabled`            | Enable autoscaling for full nodes                  | `true`                               |
| `fullNode.autoscaling.minReplicas`        | Minimum number of replicas                         | `3`                                  |
| `fullNode.autoscaling.maxReplicas`        | Maximum number of replicas                         | `10`                                 |
| `fullNode.resources`                      | CPU/Memory resource requests/limits                | See values.yaml                      |
| `miner.enabled`                           | Deploy mining nodes                                | `true`                               |
| `miner.replicaCount`                      | Number of miner replicas                           | `2`                                  |
| `miner.resources`                         | CPU/Memory resource requests/limits for miners     | See values.yaml                      |
| `prometheus.enabled`                      | Deploy Prometheus monitoring                       | `true`                               |
| `alertManager.enabled`                    | Deploy AlertManager                                | `true`                               |
| `alertManager.receivers.email.enabled`    | Enable email alerts                                | `true`                               |
| `alertManager.receivers.slack.enabled`    | Enable Slack alerts                                | `true`                               |
| `grafana.enabled`                         | Deploy Grafana dashboard                           | `true`                               |
| `grafana.adminPassword`                   | Admin password for Grafana                         | `supernova-admin-password`           |
| `backupManager.enabled`                   | Enable automated backups                           | `true`                               |
| `backupManager.schedule`                  | Backup schedule (cron format)                      | `"0 0 * * *"` (daily at midnight)    |
| `backupManager.retentionDays`             | Number of days to retain backups                   | `30`                                 |

For a complete list of configurable parameters, examine the values.yaml file.

## Production Deployment Recommendations

For production deployments, consider the following recommendations:

1. **Resource Allocation**: Adjust resource requests and limits based on your cluster's capacity and expected load.
2. **Data Persistence**: Use high-performance storage for data volumes to ensure optimal blockchain performance.
3. **Network Configuration**: Configure proper network policies to secure communication between components.
4. **Monitoring and Alerting**: Set up proper monitoring and alerting configurations with appropriate contact information.
5. **Security**: Use TLS for all ingress endpoints and secure sensitive configuration parameters.
6. **Backup Strategy**: Configure backup retention and scheduling based on your data sensitivity and regulatory requirements.

## Scaling

The Supernova chart includes horizontal pod autoscaling for full nodes to handle varying loads. Mining nodes do not use autoscaling by default, as they are typically resource-optimized for specific hardware.

## Backup and Recovery

The chart includes a backup manager that regularly backs up critical data. To restore from a backup:

1. Identify the backup you want to restore from
2. Stop the affected components
3. Use the provided restoration scripts to restore the data
4. Restart the components

## Monitoring

The chart includes Prometheus and Grafana for monitoring. Access Grafana at the provided ingress endpoint to view:

- Node health and performance
- Blockchain metrics (block height, transactions, etc.)
- System resource utilization
- Network performance

## Troubleshooting

Common issues:

1. **Insufficient Resources**: Ensure your Kubernetes cluster has sufficient resources to handle the requested allocations.
2. **Storage Issues**: Verify that the StorageClass specified in `global.storageClass` exists and can provision volumes.
3. **Networking Problems**: Check that pod-to-pod communication is working correctly, especially if network policies are in place.
4. **Monitoring Issues**: If Prometheus or Grafana are not working, check their logs for configuration problems.

## Upgrade Guide

To upgrade the Supernova deployment:

```bash
# Update the repository
helm repo update

# Upgrade the release
helm upgrade supernova supernova/supernova
```

Review the release notes before upgrading to be aware of any breaking changes or manual steps that might be required. 