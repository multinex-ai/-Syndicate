# Public release boundary

Syndicate is published from a clean, allowlisted export. The private source workspace is never made public and its Git history is never reused.

## Public package

The public repository may contain the terminal harness, provider-neutral capability contracts, local tool adapters, customer-owned provider interfaces, MemQ integration interfaces, public configuration examples, documentation, and tests.

The public client asks for capabilities such as text, image, video, audio, or authorized security analysis. It does not select a hosted model, provider endpoint, revision, worker image, or routing rule.

## Private service

The private control plane owns hosted authentication, entitlements, credit accounting, rate limits, provider credentials, endpoint selection, model and adapter catalogs, routing policy, private system prompts, cost coefficients, qualification evidence, and normalized provider error handling.

Runpod network storage may hold private model weights, adapters, caches, and large worker assets. Private worker images and runtime secrets carry execution code and credentials. Routing decisions remain in the authenticated control plane; a mounted volume is not the only secrecy boundary because any attached authorized worker can read it.

## Release gate

Prepare the public tree in a separate staging directory, then run:

```sh
node scripts/verify-public-release.mjs /absolute/path/to/prepared-syndicate-public
```

The verifier rejects private directories, Git history, owner bindings, machine paths, endpoint identifiers, private model identities, secret-like assignments, model-weight formats, escaping symlinks, and oversized files. Publication still requires owner review and approval of the exact public README, repository description, and release text.

Never use the private workspace itself as the verifier target and never change its GitHub visibility to public.
