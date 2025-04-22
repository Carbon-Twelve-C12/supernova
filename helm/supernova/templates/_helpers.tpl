{{/*
Expand the name of the chart.
*/}}
{{- define "supernova.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "supernova.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "supernova.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "supernova.labels" -}}
helm.sh/chart: {{ include "supernova.chart" . }}
{{ include "supernova.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "supernova.selectorLabels" -}}
app.kubernetes.io/name: {{ include "supernova.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "supernova.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "supernova.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create node config
*/}}
{{- define "supernova.nodeConfig" -}}
# SuperNova Node Configuration
[network]
network_name = "{{ .Values.node.config.network.networkName }}"
p2p_port = {{ .Values.node.config.network.p2pPort }}
rpc_port = {{ .Values.node.config.network.rpcPort }}
max_connections = {{ .Values.node.config.network.maxConnections }}
dns_seeds = [{{ range $i, $e := .Values.node.config.network.dnsSeeds }}{{ if $i }}, {{ end }}"{{ $e }}"{{ end }}]
is_testnet = {{ .Values.node.config.network.isTestnet }}

[consensus]
target_block_time = {{ .Values.node.config.consensus.targetBlockTime }}
initial_difficulty = {{ .Values.node.config.consensus.initialDifficulty }}
difficulty_adjustment_window = {{ .Values.node.config.consensus.difficultyAdjustmentWindow }}

[mining]
enabled = {{ .Values.node.config.mining.enabled }}

[storage]
db_path = "{{ .Values.node.config.storage.dbPath }}"
prune_mode = "{{ .Values.node.config.storage.pruneMode }}"

[telemetry]
metrics_enabled = {{ .Values.node.config.telemetry.metricsEnabled }}
metrics_port = {{ .Values.node.config.telemetry.metricsPort }}
log_level = "{{ .Values.node.config.telemetry.logLevel }}"
{{- end }} 