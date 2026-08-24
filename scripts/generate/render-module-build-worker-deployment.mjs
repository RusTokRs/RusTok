#!/usr/bin/env node

import { pathToFileURL } from 'node:url';

const DNS_LABEL = /^[a-z0-9](?:[-a-z0-9]*[a-z0-9])?$/;
const DIGEST = /^sha256:[0-9a-f]{64}$/;

function fail(message) {
  throw new Error(`[render-module-build-worker-deployment] ${message}`);
}

function integer(value, name, minimum = 1) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < minimum) fail(`${name} must be an integer >= ${minimum}`);
  return parsed;
}

function label(value, name) {
  if (value.length > 63 || !DNS_LABEL.test(value)) fail(`${name} must be a Kubernetes DNS label`);
}

export function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith('--') || value === undefined || value.startsWith('--')) fail('arguments must use --name value pairs');
    if (values.has(key)) fail(`duplicate argument ${key}`);
    values.set(key, value);
  }
  const required = (name) => {
    const value = values.get(name);
    if (!value) fail(`${name} is required`);
    return value;
  };
  const known = new Set(['--namespace', '--name', '--image', '--digest', '--runtime', '--tls-secret', '--attestation-config-map', '--config-map', '--source-pvc', '--server-label-value', '--replicas', '--port']);
  for (const key of values.keys()) if (!known.has(key)) fail(`unknown argument ${key}`);
  const result = {
    namespace: required('--namespace'),
    name: values.get('--name') ?? 'rustok-module-build-worker',
    image: required('--image'),
    digest: required('--digest'),
    runtime: required('--runtime'),
    tlsSecret: required('--tls-secret'),
    attestationConfigMap: required('--attestation-config-map'),
    configMap: required('--config-map'),
    sourcePvc: required('--source-pvc'),
    serverLabelValue: values.get('--server-label-value') ?? 'rustok-module-build-dispatcher',
    replicas: integer(values.get('--replicas') ?? '2', '--replicas', 2),
    port: integer(values.get('--port') ?? '50051', '--port'),
  };
  for (const [name, value] of Object.entries({
    '--namespace': result.namespace, '--name': result.name, '--tls-secret': result.tlsSecret,
    '--attestation-config-map': result.attestationConfigMap, '--config-map': result.configMap,
    '--source-pvc': result.sourcePvc, '--server-label-value': result.serverLabelValue,
  })) label(value, name);
  if (!DIGEST.test(result.digest)) fail('--digest must be a lowercase SHA-256 digest');
  if (!['gvisor', 'kata'].includes(result.runtime)) fail('--runtime must be exactly gvisor or kata');
  if (!result.image || /\s|@/.test(result.image)) fail('--image must be a repository without a digest');
  if (result.port > 65535) fail('--port must not exceed 65535');
  return result;
}

export function renderDeployment(options) {
  const runtimeClass = options.runtime === 'gvisor' ? 'runsc' : 'kata';
  const image = `${options.image}@${options.digest}`;
  return `apiVersion: v1
kind: ServiceAccount
metadata:
  name: ${options.name}
  namespace: ${options.namespace}
automountServiceAccountToken: false
---
apiVersion: v1
kind: Service
metadata:
  name: ${options.name}
  namespace: ${options.namespace}
spec:
  clusterIP: None
  selector:
    app.kubernetes.io/name: ${options.name}
    app.kubernetes.io/component: module-build-worker
  ports:
    - name: grpc-mtls
      port: ${options.port}
      targetPort: grpc-mtls
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${options.name}
  namespace: ${options.namespace}
spec:
  replicas: ${options.replicas}
  minReadySeconds: 10
  progressDeadlineSeconds: 300
  revisionHistoryLimit: 2
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 0
      maxSurge: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: ${options.name}
      app.kubernetes.io/component: module-build-worker
  template:
    metadata:
      labels:
        app.kubernetes.io/name: ${options.name}
        app.kubernetes.io/component: module-build-worker
      annotations:
        rustok.io/isolation-runtime: ${options.runtime}
        rustok.io/image-digest: ${options.digest}
    spec:
      serviceAccountName: ${options.name}
      automountServiceAccountToken: false
      runtimeClassName: ${runtimeClass}
      hostNetwork: false
      hostPID: false
      hostIPC: false
      enableServiceLinks: false
      terminationGracePeriodSeconds: 30
      securityContext:
        runAsNonRoot: true
        runAsUser: 10001
        runAsGroup: 10001
        seccompProfile:
          type: RuntimeDefault
      containers:
        - name: worker
          image: ${image}
          imagePullPolicy: IfNotPresent
          ports:
            - name: grpc-mtls
              containerPort: ${options.port}
          envFrom:
            - configMapRef:
                name: ${options.configMap}
          env:
            - name: RUSTOK_MODULE_BUILD_LISTEN_ADDR
              value: "0.0.0.0:${options.port}"
            - name: RUSTOK_MODULE_BUILD_SERVER_CERT_PEM
              value: /var/run/rustok/tls/server.crt
            - name: RUSTOK_MODULE_BUILD_SERVER_KEY_PEM
              value: /var/run/rustok/tls/server.key
            - name: RUSTOK_MODULE_BUILD_CLIENT_CA_PEM
              value: /var/run/rustok/tls/client-ca.crt
            - name: RUSTOK_MODULE_BUILD_JOB_RUNTIME
              value: ${options.runtime}
            - name: RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST
              value: ${options.digest}
            - name: RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION
              value: /var/run/rustok/isolation/attestation.json
            - name: RUSTOK_MODULE_BUILD_SOURCE_ROOT
              value: /var/run/rustok/source
            - name: RUSTOK_MODULE_BUILD_PROBE_ENDPOINT
              value: "https://127.0.0.1:${options.port}"
            - name: RUSTOK_MODULE_BUILD_PROBE_CLIENT_CERT_PEM
              value: /var/run/rustok/tls/probe-client.crt
            - name: RUSTOK_MODULE_BUILD_PROBE_CLIENT_KEY_PEM
              value: /var/run/rustok/tls/probe-client.key
            - name: RUSTOK_MODULE_BUILD_PROBE_SERVER_CA_PEM
              value: /var/run/rustok/tls/server-ca.crt
            - name: RUSTOK_MODULE_BUILD_PROBE_SERVER_DOMAIN
              value: ${options.name}.${options.namespace}.svc
          securityContext:
            allowPrivilegeEscalation: false
            privileged: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
          resources:
            requests:
              cpu: 500m
              memory: 256Mi
              ephemeral-storage: 1Gi
            limits:
              cpu: "2"
              memory: 512Mi
              ephemeral-storage: 2Gi
          volumeMounts:
            - { name: tls, mountPath: /var/run/rustok/tls, readOnly: true }
            - { name: isolation, mountPath: /var/run/rustok/isolation, readOnly: true }
            - { name: source, mountPath: /var/run/rustok/source, readOnly: true }
            - { name: work, mountPath: /var/lib/rustok/work }
            - { name: cargo-home, mountPath: /var/lib/rustok/cargo }
            - { name: temporary, mountPath: /tmp }
          startupProbe:
            exec: { command: ["/app/rustok-module-build-worker-probe"] }
            periodSeconds: 2
            timeoutSeconds: 5
            failureThreshold: 30
          readinessProbe:
            exec: { command: ["/app/rustok-module-build-worker-probe"] }
            periodSeconds: 5
            timeoutSeconds: 5
            failureThreshold: 2
          livenessProbe:
            exec: { command: ["/app/rustok-module-build-worker-probe"] }
            periodSeconds: 10
            timeoutSeconds: 5
            failureThreshold: 3
      volumes:
        - name: tls
          secret: { secretName: ${options.tlsSecret}, defaultMode: 256 }
        - name: isolation
          configMap:
            name: ${options.attestationConfigMap}
            defaultMode: 256
            items: [{ key: attestation.json, path: attestation.json }]
        - name: source
          persistentVolumeClaim: { claimName: ${options.sourcePvc}, readOnly: true }
        - name: work
          emptyDir: { sizeLimit: 1Gi }
        - name: cargo-home
          emptyDir: { sizeLimit: 512Mi }
        - name: temporary
          emptyDir: { medium: Memory, sizeLimit: 128Mi }
---
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: ${options.name}
  namespace: ${options.namespace}
spec:
  minAvailable: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: ${options.name}
      app.kubernetes.io/component: module-build-worker
---
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: ${options.name}
  namespace: ${options.namespace}
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: ${options.name}
      app.kubernetes.io/component: module-build-worker
  policyTypes: ["Ingress", "Egress"]
  ingress:
    - from:
        - podSelector:
            matchLabels:
              app.kubernetes.io/name: ${options.serverLabelValue}
      ports:
        - { protocol: TCP, port: ${options.port} }
  egress: []
`;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { process.stdout.write(renderDeployment(parseArguments(process.argv.slice(2)))); }
  catch (error) { console.error(error instanceof Error ? error.message : String(error)); process.exitCode = 1; }
}
