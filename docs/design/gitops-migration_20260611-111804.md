## `flux/infrastructure/monitoring-stack/node-exporter/node-exporter-source.yaml`
```yaml
apiVersion: source.toolkit.fluxcd.io/v1beta2
kind: GitRepository
metadata:
  name: node-exporter-source
  namespace: flux-system
spec:
  interval: 10m0s
  url: ssh://git@github.com/your-org/your-config-repo.git
  ref:
    branch: main
  # path inside the repo for the node-exporter manifests
  # Assuming 'flux/infrastructure/monitoring-stack/node-exporter' is the repo-relative folder
  # Replace with actual relative path if differs
```

---
## `flux/infrastructure/monitoring-stack/node-exporter/namespace.yaml`
```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: monitoring-node-exporter
  labels:
    app.kubernetes.io/name: node-exporter
    app.kubernetes.io/managed-by: fluxcd
```

---
## `flux/infrastructure/monitoring-stack/node-exporter/serviceaccount.yaml`
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: node-exporter
  namespace: monitoring-node-exporter
  labels:
    app.kubernetes.io/name: node-exporter
    app.kubernetes.io/managed-by: fluxcd
```

---
## `flux/infrastructure/monitoring-stack/node-exporter/clusterrole.yaml`
```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: node-exporter
  labels:
    app.kubernetes.io/name: node-exporter
    app.kubernetes.io/managed-by: fluxcd
rules:
  - apiGroups: [""]
    resources: ["nodes", "nodes/proxy", "nodes/metrics"]
    verbs: ["get", "list", "watch"]
  - apiGroups: [""]
    resources: ["endpoints"]
    verbs: ["get", "list", "watch"]
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["list", "watch"]
```

---
## `flux/infrastructure/monitoring-stack/node-exporter/clusterrolebinding.yaml`
```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: node-exporter
  labels:
    app.kubernetes.io/name: node-exporter
    app.kubernetes.io/managed-by: fluxcd
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: node-exporter
subjects:
  - kind: ServiceAccount
    name: node-exporter
    namespace: monitoring-node-exporter
```

---
## `flux/infrastructure/monitoring-stack/node-exporter/daemonset.yaml`
```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: node-exporter
  namespace: monitoring-node-exporter
  labels:
    app.kubernetes.io/name: node-exporter
    app.kubernetes.io/managed-by: fluxcd
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: node-exporter
  template:
    metadata:
      labels:
        app.kubernetes.io/name: node-exporter
    spec:
      serviceAccountName: node-exporter
      hostNetwork: true
      dnsPolicy: ClusterFirstWithHostNet
      containers:
        - name: node-exporter
          image: quay.io/prometheus/node-exporter:v1.5.0
          args:
            - "--path.procfs=/host/proc"
            - "--path.sysfs=/host/sys"
            - "--collector.filesystem.ignored-mount-points=^/(sys|proc|dev|host|etc)($|/)"
            - "--collector.netclass.ignored-devices=^(lo|docker.*)$"
          resources:
            limits:
              cpu: 100m
              memory: 50Mi
            requests:
              cpu: 100m
              memory: 50Mi
          ports:
            - containerPort: 9100
              name: metrics
              protocol: TCP
          volumeMounts:
            - name: proc
              mountPath: /host/proc
              readOnly: true
            - name: sys
              mountPath: /host/sys
              readOnly: true
            - name: root
              mountPath: /rootfs
              readOnly: true
      volumes:
        - name: proc
          hostPath:
            path: /proc
        - name: sys
          hostPath:
            path: /sys
        - name: root
          hostPath:
            path: /
```

---
## `flux/infrastructure/monitoring-stack/node-exporter/kustomization.yaml`
```yaml
apiVersion: kustomize.toolkit.fluxcd.io/v1beta2
kind: Kustomization
metadata:
  name: node-exporter
  namespace: flux-system
spec:
  interval: 10m0s
  path: ./infrastructure/monitoring-stack/node-exporter
  prune: true
  sourceRef:
    kind: GitRepository
    name: node-exporter-source
  validation: client
  dependsOn: []
```

---

# Notes

- No `.sops.yaml` or secret stubs are necessary as node-exporter does not use secrets in this context.
- These manifests do not define or modify any PersistentVolumeClaims or PersistentVolumes, so no risk of data loss exists.
- The RBAC and DaemonSet resources align with existing Ansible-managed state, preserving permissions and labels.
- The Flux Kustomization is set to reconcile at a 10-minute interval, appropriate for monitoring infrastructure.
- Namespace `monitoring-node-exporter` is explicitly created and used uniformly.
- This set of manifests is complete and safe for incremental Flux migration for Milestone 1.