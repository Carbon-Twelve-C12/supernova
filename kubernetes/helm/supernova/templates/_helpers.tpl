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
Create the node configuration as TOML
*/}}
{{- define "supernova.nodeConfig" -}}
# SuperNova Node Configuration
[network]
network_name = {{ .Values.network.networkName | quote }}
p2p_port = {{ .Values.network.p2pPort }}
rpc_port = {{ .Values.network.rpcPort }}
max_connections = {{ .Values.network.maxConnections }}
dns_seeds = [
{{- range $index, $seed := .Values.network.dnsSeeds }}
  {{- if $index }},{{ end }}{{ $seed | quote }}
{{- end }}
]
is_testnet = {{ .Values.network.isTestnet }}

[consensus]
target_block_time = {{ .Values.consensus.targetBlockTime }}
initial_difficulty = {{ .Values.consensus.initialDifficulty }}
difficulty_adjustment_window = {{ .Values.consensus.difficultyAdjustmentWindow }}

[storage]
db_path = {{ .Values.storage.dbPath | quote }}
prune_mode = {{ .Values.storage.pruneMode | quote }}

[telemetry]
metrics_enabled = {{ .Values.telemetry.metricsEnabled }}
metrics_port = {{ .Values.telemetry.metricsPort }}
log_level = {{ .Values.telemetry.logLevel | quote }}

[checkpoint]
checkpoints_enabled = {{ .Values.checkpoint.checkpointsEnabled }}
checkpoint_interval = {{ .Values.checkpoint.checkpointInterval }}
checkpoint_type = {{ .Values.checkpoint.checkpointType | quote }}
data_dir = {{ .Values.checkpoint.dataDir | quote }}
max_checkpoints = {{ .Values.checkpoint.maxCheckpoints }}

[backup]
backup_dir = {{ .Values.backup.backupDir | quote }}
max_backups = {{ .Values.backup.maxBackups }}
backup_interval = {{ .Values.backup.backupInterval }}
enable_automated_backups = {{ .Values.backup.enableAutomatedBackups }}
verify_on_startup = {{ .Values.backup.verifyOnStartup }}
{{- end }} 