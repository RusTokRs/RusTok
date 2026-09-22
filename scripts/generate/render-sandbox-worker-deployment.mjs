#!/usr/bin/env node

import { pathToFileURL } from 'node:url';

const DNS_LABEL = /^[a-z0-9](?:[-a-z0-9]*[a-z0-9])?$/;
const LABEL_KEY = /^(?:[a-z0-9](?:[-a-z0-9.]*[a-z0-9])?\/)?[A-Za-z0-9](?:[-A-Za-z0-9_.]*[A-Za-z0-9])?$/;
const DIGEST = /^sha256:[0-9a-f]{64}$/;

function fail(message) {
  throw new Error(`[render-sandbox-worker-deployment] ${message}`);
}

function quote(value) {
  return JSON.stringify(String(value));
}

function parsePositiveInteger(value, name, minimum = 1) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < minimum) {
    fail(`${name} must be an integer greater than or equal to ${minimum}`);
  }
  return parsed;
}

function validateDnsLabel(value, name) {
  if (value.length > 63 || !DNS_LABEL.test(value)) {
    fail(`${name} must be a Kubernetes DNS label`);
  }
}

export function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith('--') || value === undefined || value.startsWith('--')) {
      fail('arguments must use --name value pairs');
    }
    if (values.has(key)) fail(`duplicate argument ${key}`);
    values.set(key, value);
  }
  const required = (name) => {
    const value = values.get(name);
    if (!value) fail(`${name} is required`);
    return value;
  };
  const known = new Set([
    '--namespace',
    '--name',
    '--image',
    '--digest',
    '--runtime',
    '--tls-secret',
    '--attestation-config-map',
    '--server-label-key',
    '--server-label-value',
    '--replicas',
    '--port',
  ]);
  for (const key of values.keys()) {
    if (!known.has(key)) fail(`unknown argument ${key}`);
  }

  const result = {
    namespace: required('--namespace'),
    name: values.get('--name') ?? 'rustok-sandbox-worker',
    image: required('--image'),
    digest: required('--digest'),
    runtime: required('--runtime'),
    tlsSecret: required('--tls-secret'),
    attestationConfigMap: required('--attestation-config-map'),
    serverLabelKey: values.get('--server-label-key') ?? 'app.kubernetes.io/name',
    serverLabelValue: values.get('--server-label-value') ?? 'rustok-server',
    replicas: parsePositiveInteger(values.get('--replicas') ?? '2', '--replicas', 2),
    port: parsePositiveInteger(values.get('--port') ?? '50051', '--port'),
  };
  for (const [value, name] of [
    [result.namespace, '--namespace'],
    [result.name, '--name'],
    [result.tlsSecret, '--tls-secret'],
    [result.attestationConfigMap, '--attestation-config-map'],
  ]) {
    validateDnsLabel(value, name);
  }
  if (!DIGEST.test(result.digest)) fail('--digest must be a lowercase SHA-256 digest');
  if (!['gvisor', 'kata'].includes(result.runtime)) {
    fail('--runtime must be exactly gvisor or kata');
  }
  if (!result.image || /\s|@/.test(result.image)) {
    fail('--image must be a repository without a digest');
  }
  if (!LABEL_KEY.test(result.serverLabelKey)) {
    fail('--server-label-key must be a Kubernetes label key');
  }
  validateDnsLabel(result.serverLabelValue, '--server-label-value');
  if (result.port > 65_535) fail('--port must not exceed 65535');
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
    app.kubernetes.io/component: sandbox-worker
  ports:
    - name: grpc-mtls
      port: ${options.port}
      targetPort: grpc-mtls
      protocol: TCP
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
      app.kubernetes.io/component: sandbox-worker
  template:
    metadata:
      labels:
        app.kubernetes.io/name: ${options.name}
        app.kubernetes.io/component: sandbox-worker
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
      terminationGracePeriodSeconds: 10
      securityContext:
        runAsNonRoot: true
        runAsUser: 10001
        runAsGroup: 10001
        seccompProfile:
          type: RuntimeDefault
      topologySpreadConstraints:
        - maxSkew: 1
          topologyKey: kubernetes.io/hostname
          whenUnsatisfiable: DoNotSchedule
          labelSelector:
            matchLabels:
              app.kubernetes.io/name: ${options.name}
              app.kubernetes.io/component: sandbox-worker
      containers:
        - name: worker
          image: ${image}
          imagePullPolicy: IfNotPresent
          ports:
            - name: grpc-mtls
              containerPort: ${options.port}
              protocol: TCP
          env:
            - name: RUSTOK_SANDBOX_LISTEN_ADDR
              value: ${quote(`0.0.0.0:${options.port}`)}
            - name: RUSTOK_SANDBOX_SERVER_CERT_PEM
              value: /var/run/rustok/tls/server.crt
            - name: RUSTOK_SANDBOX_SERVER_KEY_PEM
              value: /var/run/rustok/tls/server.key
            - name: RUSTOK_SANDBOX_CLIENT_CA_PEM
              value: /var/run/rustok/tls/client-ca.crt
            - name: RUSTOK_SANDBOX_RUNTIME
              value: ${options.runtime}
            - name: RUSTOK_SANDBOX_IMAGE_DIGEST
              value: ${options.digest}
            - name: RUSTOK_SANDBOX_ISOLATION_ATTESTATION
              value: /var/run/rustok/isolation/attestation.json
            - name: RUSTOK_SANDBOX_PROBE_ENDPOINT
              value: ${quote(`https://127.0.0.1:${options.port}`)}
            - name: RUSTOK_SANDBOX_PROBE_CLIENT_CERT_PEM
              value: /var/run/rustok/tls/probe-client.crt
            - name: RUSTOK_SANDBOX_PROBE_CLIENT_KEY_PEM
              value: /var/run/rustok/tls/probe-client.key
            - name: RUSTOK_SANDBOX_PROBE_SERVER_CA_PEM
              value: /var/run/rustok/tls/server-ca.crt
            - name: RUSTOK_SANDBOX_PROBE_SERVER_DOMAIN
              value: ${options.name}.${options.namespace}.svc
            - name: RUSTOK_SANDBOX_CONCURRENCY_LIMIT
              value: "1"
          securityContext:
            allowPrivilegeEscalation: false
            privileged: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
          resources:
            requests:
              cpu: 250m
              memory: 64Mi
              ephemeral-storage: 64Mi
            limits:
              cpu: "1"
              memory: 128Mi
              ephemeral-storage: 64Mi
          volumeMounts:
            - name: tls
              mountPath: /var/run/rustok/tls
              readOnly: true
            - name: isolation
              mountPath: /var/run/rustok/isolation
              readOnly: true
            - name: temporary
              mountPath: /tmp
          startupProbe:
            exec:
              command: ["/app/rustok-sandbox-worker-probe"]
            periodSeconds: 2
            timeoutSeconds: 3
            failureThreshold: 30
          readinessProbe:
            exec:
              command: ["/app/rustok-sandbox-worker-probe"]
            periodSeconds: 5
            timeoutSeconds: 3
            failureThreshold: 2
          livenessProbe:
            exec:
              command: ["/app/rustok-sandbox-worker-probe"]
            periodSeconds: 10
            timeoutSeconds: 3
            failureThreshold: 3
      volumes:
        - name: tls
          secret:
            secretName: ${options.tlsSecret}
            defaultMode: 256
        - name: isolation
          configMap:
            name: ${options.attestationConfigMap}
            defaultMode: 256
            items:
              - key: attestation.json
                path: attestation.json
        - name: temporary
          emptyDir:
            medium: Memory
            sizeLimit: 64Mi
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
      app.kubernetes.io/component: sandbox-worker
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
      app.kubernetes.io/component: sandbox-worker
  policyTypes: ["Ingress", "Egress"]
  ingress:
    - from:
        - podSelector:
            matchLabels:
              ${options.serverLabelKey}: ${options.serverLabelValue}
      ports:
        - protocol: TCP
          port: ${options.port}
  egress: []
`;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    process.stdout.write(renderDeployment(parseArguments(process.argv.slice(2))));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
