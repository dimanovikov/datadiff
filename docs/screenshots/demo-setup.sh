# Run from the repository root after `cargo build --release`.
export PATH="$PWD/target/release:$PATH"
export PS1='\[\e[1;34m\]$\[\e[0m\] '
cd "$(mktemp -d)" && git init -q && git config user.email demo@example.com && git config user.name demo && git config core.pager cat && git config color.ui always
cat > deploy.yaml <<'YAML'
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: api
          image: app:1.0
        - name: sidecar
          image: envoy:1.29
YAML
git add deploy.yaml && git commit -q -m v1
cat > deploy.yaml <<'YAML'
kind: Deployment
spec:
  template:
    spec:
      containers:
        - image: envoy:1.29
          name: sidecar
        - image: app:1.1
          name: api
  replicas: 5
metadata:
  name: api
apiVersion: apps/v1
YAML
git config diff.datadiff.command "datadiff --key name git-diff"
echo '*.yaml diff=datadiff' > .gitattributes
