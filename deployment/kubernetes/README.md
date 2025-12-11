# Supernova Kubernetes Deployment

This directory contains Kubernetes manifests for deploying a 3-node Supernova testnet cluster.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Supernova Testnet Cluster                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │   Node 0    │  │   Node 1    │  │   Node 2    │              │
│  │  (Leader)   │──│  (Follower) │──│  (Follower) │              │
│  │             │  │             │  │             │              │
│  │ P2P: 8333   │  │ P2P: 8333   │  │ P2P: 8333   │              │
│  │ RPC: 8332   │  │ RPC: 8332   │  │ RPC: 8332   │              │
│  │ Metrics:9100│  │ Metrics:9100│  │ Metrics:9100│              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
│         │                │                │                      │
│         └────────────────┼────────────────┘                      │
│                          │                                       │
│                    ┌─────┴─────┐                                 │
│                    │  Services │                                 │
│                    └───────────┘                                 │
│                                                                  │
│  Services:                                                       │
│  - supernova-headless: Internal DNS for pod discovery            │
│  - supernova-rpc: ClusterIP for internal RPC access              │
│  - supernova-p2p: LoadBalancer for external P2P connections      │
│  - supernova-rpc-external: NodePort for external RPC access      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Prerequisites

- Kubernetes cluster (v1.25+)
- kubectl configured with cluster access
- Storage class available (default: `standard`)
- Docker image `supernova/node:latest` available

## Quick Start

### 1. Deploy using Kustomize

```bash
# Deploy all resources
kubectl apply -k deployment/kubernetes/

# Verify deployment
kubectl get all -n supernova-testnet
```

### 2. Deploy manually (alternative)

```bash
# Create namespace
kubectl apply -f deployment/kubernetes/namespace.yaml

# Create RBAC resources
kubectl apply -f deployment/kubernetes/rbac.yaml

# Create ConfigMap
kubectl apply -f deployment/kubernetes/configmap.yaml

# Create PodDisruptionBudget
kubectl apply -f deployment/kubernetes/pdb.yaml

# Create NetworkPolicy
kubectl apply -f deployment/kubernetes/network-policy.yaml

# Create Services
kubectl apply -f deployment/kubernetes/service.yaml

# Create StatefulSet
kubectl apply -f deployment/kubernetes/statefulset.yaml
```

## Verification

### Check Pod Status

```bash
# List all pods
kubectl get pods -n supernova-testnet -w

# Expected output (after a few minutes):
# NAME                READY   STATUS    RESTARTS   AGE
# supernova-node-0    1/1     Running   0          5m
# supernova-node-1    1/1     Running   0          4m
# supernova-node-2    1/1     Running   0          3m
```

### Check Node Health

```bash
# Check liveness of node-0
kubectl exec -n supernova-testnet supernova-node-0 -- curl -s http://localhost:8332/health/live

# Check readiness of node-0
kubectl exec -n supernova-testnet supernova-node-0 -- curl -s http://localhost:8332/health/ready
```

### View Logs

```bash
# Stream logs from node-0
kubectl logs -n supernova-testnet supernova-node-0 -f

# View logs from all nodes
kubectl logs -n supernova-testnet -l app.kubernetes.io/name=supernova
```

### Check Peer Connections

```bash
# Get peer info from node-0
kubectl exec -n supernova-testnet supernova-node-0 -- \
  curl -s http://localhost:8332 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getpeerinfo","params":[],"id":1}'
```

## Accessing the Testnet

### Internal RPC Access (from within cluster)

```bash
# From any pod in the cluster
curl http://supernova-rpc.supernova-testnet.svc.cluster.local:8332/health/ready
```

### External RPC Access (NodePort)

```bash
# Get the NodePort
kubectl get svc supernova-rpc-external -n supernova-testnet

# Access from outside (replace <node-ip> with any cluster node IP)
curl http://<node-ip>:30332/health/ready
```

### External P2P Access (LoadBalancer)

```bash
# Get the external IP
kubectl get svc supernova-p2p -n supernova-testnet

# Connect external nodes to this IP on port 8333
```

## Scaling

### Scale to more nodes

```bash
# Scale to 5 nodes
kubectl scale statefulset supernova-node -n supernova-testnet --replicas=5

# Scale down (nodes will be removed in reverse order)
kubectl scale statefulset supernova-node -n supernova-testnet --replicas=3
```

**Note:** Update the PodDisruptionBudget `minAvailable` when changing replica count.

## Configuration

### Modifying Node Configuration

Edit `configmap.yaml` and apply:

```bash
kubectl apply -f deployment/kubernetes/configmap.yaml

# Restart pods to pick up new config
kubectl rollout restart statefulset/supernova-node -n supernova-testnet
```

### Resource Limits

Default resources per node:
- CPU: 1 core request, 2 cores limit
- Memory: 2Gi request, 4Gi limit
- Storage: 100Gi persistent volume

Modify in `statefulset.yaml` under `resources` section.

## Monitoring

### Prometheus Integration

The pods are annotated for Prometheus scraping:

```yaml
prometheus.io/scrape: "true"
prometheus.io/port: "9100"
prometheus.io/path: "/metrics"
```

### Grafana Dashboards

Import the dashboard from `deployment/monitoring/grafana/dashboards/supernova-overview.json`.

## Troubleshooting

### Pod not starting

```bash
# Check events
kubectl describe pod supernova-node-0 -n supernova-testnet

# Check storage
kubectl get pvc -n supernova-testnet
```

### Nodes not connecting

```bash
# Check DNS resolution
kubectl exec -n supernova-testnet supernova-node-0 -- \
  nslookup supernova-node-1.supernova-headless.supernova-testnet.svc.cluster.local

# Check network policy
kubectl describe networkpolicy -n supernova-testnet
```

### Data persistence issues

```bash
# Check PVC status
kubectl get pvc -n supernova-testnet

# Check storage class
kubectl get storageclass
```

## Cleanup

```bash
# Delete all resources (preserves PVCs)
kubectl delete -k deployment/kubernetes/

# Delete PVCs (WARNING: deletes all blockchain data)
kubectl delete pvc -n supernova-testnet --all

# Delete namespace
kubectl delete namespace supernova-testnet
```

## Security Considerations

1. **NetworkPolicy**: Restricts traffic to necessary ports only
2. **RBAC**: Minimal permissions for node service account
3. **SecurityContext**: Non-root user, read-only filesystem
4. **Pod Security**: No privilege escalation, dropped capabilities
5. **PodDisruptionBudget**: Ensures availability during maintenance

## Production Recommendations

1. Use dedicated storage class with SSD backing
2. Enable pod anti-affinity across availability zones
3. Configure resource limits based on actual usage
4. Set up proper monitoring and alerting
5. Use secrets management (e.g., HashiCorp Vault) for sensitive data
6. Enable TLS for RPC endpoints
7. Configure backup strategy for persistent volumes

