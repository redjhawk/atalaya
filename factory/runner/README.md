# Factory runner on k3s

Self-hosted runner for the factory's agent runs, on a k3s cluster in the home network. GitHub Actions stays the orchestrator; only the runner moves. It uses Actions Runner Controller (ARC): each run gets a fresh pod that is deleted when the run ends, and no pod exists while nothing is queued.

Nothing here has been deployed or tested yet.

## Before installing: the repository is public

A self-hosted runner on a public repository can be made to run a stranger's code: a pull request from a fork can change a workflow to `runs-on: atalaya-factory`. Do both of these first:

1. In the repository settings, under Actions, General, set fork pull request workflows to **require approval for all external contributors**. Never approve a run whose diff touches `.github/workflows/`.
2. Apply the network policy below, so that a run cannot reach the home network even if it is hijacked.

Making the repository private removes the risk altogether.

Workflows triggered by `pull_request` stay on GitHub-hosted runners. Only workflows a repository owner triggers, such as labelling an issue, may use `runs-on: atalaya-factory`.

## Install

1. Create a fine-grained personal access token limited to this repository, with **Administration: read and write**. ARC uses it to register runners.

2. Create the namespace, the secret and the network policy:

   ```sh
   kubectl create namespace atalaya-runners
   kubectl -n atalaya-runners create secret generic atalaya-runner-github \
     --from-literal=github_token='<token>'
   kubectl apply -f factory/runner/networkpolicy.yaml
   ```

3. Install the controller once per cluster, then the scale set:

   ```sh
   helm install arc \
     --namespace arc-systems --create-namespace \
     oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set-controller

   helm install atalaya-factory \
     --namespace atalaya-runners \
     -f factory/runner/values.yaml \
     oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set
   ```

4. Check that the runner set `atalaya-factory` appears in the repository settings, under Actions, Runners.

## Verify the isolation

Start a throwaway pod in `atalaya-runners` and confirm that it reaches `https://api.github.com` and does not reach an address in the home network, such as the router. If the home address answers, the cluster is not enforcing network policies and the runner must not be used.

## Notes

- The Claude credential is not on the cluster. It stays a GitHub repository secret and is handed to each run.
- The stock runner image may not include the `gh` CLI; the ticketing workflow downloads it when missing. That download assumes an x86_64 node.
- To stop the factory: `helm uninstall atalaya-factory -n atalaya-runners`. Queued runs then wait and eventually time out.
