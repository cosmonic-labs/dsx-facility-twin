# Security notes (lab build)

- **Login**: `admin/admin` sets a fixed session cookie (`TWIN_SESSION_TOKEN`). It keeps casual
  visitors out of a lab VM. It is not authentication. Put the API behind the site's IdP (OIDC at
  the ingress, or the DSX Agent Gateway's JWT policy) before anyone else can reach it.
- **Egress**: only `twin-api` has an allowed host (the simulator). The analytics components have
  none.
- **NATS grants**: each workload is limited to its subjects and the `twin` bucket.
- **Registry**: the `twin` hostgroup pulls over plain HTTP from the kind registry. Use a TLS
  registry (`caBundle`) outside the lab.
