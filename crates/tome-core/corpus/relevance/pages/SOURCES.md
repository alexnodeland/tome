# Relevance corpus — sources

Per [`corpus/README.md`](../README.md), every committed page records where it
came from and under which licence it is redistributed. This is the SPIKE-010
licence gate applied to the relevance corpus, which ships publicly like the
rest of the repository.

**339 pages across six documentation platforms**, fetched 2026-07-29.

## How these were produced

By running the real `tome pull` against the sections listed in
[`../corpus.yaml`](../corpus.yaml) — so `robots.txt` was honoured for every
host, requests were rate-limited to 2/s and issued one at a time, and the
crawler's self-identifying user agent
(`Tome/0.0.0 (+https://github.com/alexnodeland/tome)`) was used throughout.

Each `pages/<source>/<path>.json` is the serialized `store::StoredPage` that
`pipeline::pull` wrote: **parsed, normalized, sanitized, and asset-localized**
— not the bytes as served. That is a derived work rather than a copy, and
every licence below permits redistribution of modified works.

**No new host was introduced.** These are the same six sources already
verified for the normalization corpus, and reusing them is deliberate: it
meant no new licence review was required. Adding a host here requires
re-running that review and updating this file.

## Licences, as stated by each source

| Source | Licence | Verified from |
|---|---|---|
| docs.python.org | **PSF-2.0** (code samples additionally 0BSD) | Page footer: "This page is licensed under the Python Software Foundation License Version 2." |
| doc.rust-lang.org (Cargo book, `std`) | **MIT OR Apache-2.0** | `rust-lang/cargo` README § License |
| nodejs.org/api | **MIT** | `nodejs/node` `LICENSE` |
| kubernetes.io | **CC-BY-4.0** | Page footer: "Documentation Distributed under CC BY 4.0" |
| go.dev | **CC-BY-4.0** (code samples BSD) | `go.dev/copyright` |

Every one permits alteration and redistribution, which is the gate in
`corpus/README.md`. **No CC-BY-SA input is present**, so the derived pages
carry no share-alike obligation — if that ever changes, this paragraph must
change with it.

## Pages


### `cargo` — doc.rust-lang.org/cargo (55 pages)

| Path | URL |
|---|---|
| `cargo/CHANGELOG.html` | https://doc.rust-lang.org/cargo/CHANGELOG.html |
| `cargo/appendix/git-authentication.html` | https://doc.rust-lang.org/cargo/appendix/git-authentication.html |
| `cargo/appendix/glossary.html` | https://doc.rust-lang.org/cargo/appendix/glossary.html |
| `cargo/commands/build-commands.html` | https://doc.rust-lang.org/cargo/commands/build-commands.html |
| `cargo/commands/cargo-build.html` | https://doc.rust-lang.org/cargo/commands/cargo-build.html |
| `cargo/commands/cargo-run.html` | https://doc.rust-lang.org/cargo/commands/cargo-run.html |
| `cargo/commands/cargo-test.html` | https://doc.rust-lang.org/cargo/commands/cargo-test.html |
| `cargo/commands/deprecated-and-removed.html` | https://doc.rust-lang.org/cargo/commands/deprecated-and-removed.html |
| `cargo/commands/general-commands.html` | https://doc.rust-lang.org/cargo/commands/general-commands.html |
| `cargo/commands/index.html` | https://doc.rust-lang.org/cargo/commands/index.html |
| `cargo/commands/manifest-commands.html` | https://doc.rust-lang.org/cargo/commands/manifest-commands.html |
| `cargo/commands/package-commands.html` | https://doc.rust-lang.org/cargo/commands/package-commands.html |
| `cargo/commands/publishing-commands.html` | https://doc.rust-lang.org/cargo/commands/publishing-commands.html |
| `cargo/commands/report-commands.html` | https://doc.rust-lang.org/cargo/commands/report-commands.html |
| `cargo/faq.html` | https://doc.rust-lang.org/cargo/faq.html |
| `cargo/getting-started/first-steps.html` | https://doc.rust-lang.org/cargo/getting-started/first-steps.html |
| `cargo/getting-started/index.html` | https://doc.rust-lang.org/cargo/getting-started/index.html |
| `cargo/getting-started/installation.html` | https://doc.rust-lang.org/cargo/getting-started/installation.html |
| `cargo/guide/build-performance.html` | https://doc.rust-lang.org/cargo/guide/build-performance.html |
| `cargo/guide/cargo-home.html` | https://doc.rust-lang.org/cargo/guide/cargo-home.html |
| `cargo/guide/cargo-toml-vs-cargo-lock.html` | https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html |
| `cargo/guide/continuous-integration.html` | https://doc.rust-lang.org/cargo/guide/continuous-integration.html |
| `cargo/guide/creating-a-new-project.html` | https://doc.rust-lang.org/cargo/guide/creating-a-new-project.html |
| `cargo/guide/dependencies.html` | https://doc.rust-lang.org/cargo/guide/dependencies.html |
| `cargo/guide/index.html` | https://doc.rust-lang.org/cargo/guide/index.html |
| `cargo/guide/project-layout.html` | https://doc.rust-lang.org/cargo/guide/project-layout.html |
| `cargo/guide/tests.html` | https://doc.rust-lang.org/cargo/guide/tests.html |
| `cargo/guide/why-cargo-exists.html` | https://doc.rust-lang.org/cargo/guide/why-cargo-exists.html |
| `cargo/guide/working-on-an-existing-project.html` | https://doc.rust-lang.org/cargo/guide/working-on-an-existing-project.html |
| `cargo/index.html` | https://doc.rust-lang.org/cargo/index.html |
| `cargo/print.html` | https://doc.rust-lang.org/cargo/print.html |
| `cargo/reference/build-cache.html` | https://doc.rust-lang.org/cargo/reference/build-cache.html |
| `cargo/reference/build-script-examples.html` | https://doc.rust-lang.org/cargo/reference/build-script-examples.html |
| `cargo/reference/build-scripts.html` | https://doc.rust-lang.org/cargo/reference/build-scripts.html |
| `cargo/reference/cargo-targets.html` | https://doc.rust-lang.org/cargo/reference/cargo-targets.html |
| `cargo/reference/config.html` | https://doc.rust-lang.org/cargo/reference/config.html |
| `cargo/reference/credential-provider-protocol.html` | https://doc.rust-lang.org/cargo/reference/credential-provider-protocol.html |
| `cargo/reference/environment-variables.html` | https://doc.rust-lang.org/cargo/reference/environment-variables.html |
| `cargo/reference/external-tools.html` | https://doc.rust-lang.org/cargo/reference/external-tools.html |
| `cargo/reference/features-examples.html` | https://doc.rust-lang.org/cargo/reference/features-examples.html |
| `cargo/reference/features.html` | https://doc.rust-lang.org/cargo/reference/features.html |
| `cargo/reference/index.html` | https://doc.rust-lang.org/cargo/reference/index.html |
| `cargo/reference/manifest.html` | https://doc.rust-lang.org/cargo/reference/manifest.html |
| `cargo/reference/overriding-dependencies.html` | https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html |
| `cargo/reference/pkgid-spec.html` | https://doc.rust-lang.org/cargo/reference/pkgid-spec.html |
| `cargo/reference/profiles.html` | https://doc.rust-lang.org/cargo/reference/profiles.html |
| `cargo/reference/publishing.html` | https://doc.rust-lang.org/cargo/reference/publishing.html |
| `cargo/reference/registries.html` | https://doc.rust-lang.org/cargo/reference/registries.html |
| `cargo/reference/registry-authentication.html` | https://doc.rust-lang.org/cargo/reference/registry-authentication.html |
| `cargo/reference/resolver.html` | https://doc.rust-lang.org/cargo/reference/resolver.html |
| `cargo/reference/rust-version.html` | https://doc.rust-lang.org/cargo/reference/rust-version.html |
| `cargo/reference/source-replacement.html` | https://doc.rust-lang.org/cargo/reference/source-replacement.html |
| `cargo/reference/specifying-dependencies.html` | https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html |
| `cargo/reference/unstable.html` | https://doc.rust-lang.org/cargo/reference/unstable.html |
| `cargo/reference/workspaces.html` | https://doc.rust-lang.org/cargo/reference/workspaces.html |

### `go` — go.dev (21 pages)

| Path | URL |
|---|---|
| `doc/articles/wiki/index.html` | https://go.dev/doc/articles/wiki/index.html |
| `doc/build-cover` | https://go.dev/doc/build-cover |
| `doc/code` | https://go.dev/doc/code |
| `doc/comment` | https://go.dev/doc/comment |
| `doc/devel/release` | https://go.dev/doc/devel/release |
| `doc/diagnostics` | https://go.dev/doc/diagnostics |
| `doc/editors` | https://go.dev/doc/editors |
| `doc/effective_go` | https://go.dev/doc/effective_go |
| `doc/faq` | https://go.dev/doc/faq |
| `doc/gc-guide` | https://go.dev/doc/gc-guide |
| `doc/index.html` | https://go.dev/doc/index.html |
| `doc/install` | https://go.dev/doc/install |
| `doc/modules/gomod-ref` | https://go.dev/doc/modules/gomod-ref |
| `doc/modules/managing-dependencies` | https://go.dev/doc/modules/managing-dependencies |
| `doc/pgo` | https://go.dev/doc/pgo |
| `doc/tutorial/create-module` | https://go.dev/doc/tutorial/create-module |
| `doc/tutorial/fuzz` | https://go.dev/doc/tutorial/fuzz |
| `doc/tutorial/generics` | https://go.dev/doc/tutorial/generics |
| `doc/tutorial/getting-started` | https://go.dev/doc/tutorial/getting-started |
| `doc/tutorial/web-service-gin` | https://go.dev/doc/tutorial/web-service-gin |
| `doc/tutorial/workspaces` | https://go.dev/doc/tutorial/workspaces |

### `kubernetes` — kubernetes.io (55 pages)

| Path | URL |
|---|---|
| `docs/concepts/architecture/cgroups/index.html` | https://kubernetes.io/docs/concepts/architecture/cgroups/index.html |
| `docs/concepts/architecture/cloud-controller/index.html` | https://kubernetes.io/docs/concepts/architecture/cloud-controller/index.html |
| `docs/concepts/architecture/control-plane-node-communication/index.html` | https://kubernetes.io/docs/concepts/architecture/control-plane-node-communication/index.html |
| `docs/concepts/architecture/controller/index.html` | https://kubernetes.io/docs/concepts/architecture/controller/index.html |
| `docs/concepts/architecture/garbage-collection/index.html` | https://kubernetes.io/docs/concepts/architecture/garbage-collection/index.html |
| `docs/concepts/architecture/index.html` | https://kubernetes.io/docs/concepts/architecture/index.html |
| `docs/concepts/architecture/leases/index.html` | https://kubernetes.io/docs/concepts/architecture/leases/index.html |
| `docs/concepts/architecture/mixed-version-proxy/index.html` | https://kubernetes.io/docs/concepts/architecture/mixed-version-proxy/index.html |
| `docs/concepts/architecture/nodes/index.html` | https://kubernetes.io/docs/concepts/architecture/nodes/index.html |
| `docs/concepts/architecture/self-healing/index.html` | https://kubernetes.io/docs/concepts/architecture/self-healing/index.html |
| `docs/concepts/containers/container-environment/index.html` | https://kubernetes.io/docs/concepts/containers/container-environment/index.html |
| `docs/concepts/containers/images/index.html` | https://kubernetes.io/docs/concepts/containers/images/index.html |
| `docs/concepts/containers/index.html` | https://kubernetes.io/docs/concepts/containers/index.html |
| `docs/concepts/containers/runtime-class/index.html` | https://kubernetes.io/docs/concepts/containers/runtime-class/index.html |
| `docs/concepts/index.html` | https://kubernetes.io/docs/concepts/index.html |
| `docs/concepts/overview/components/index.html` | https://kubernetes.io/docs/concepts/overview/components/index.html |
| `docs/concepts/overview/index.html` | https://kubernetes.io/docs/concepts/overview/index.html |
| `docs/concepts/overview/kubectl/index.html` | https://kubernetes.io/docs/concepts/overview/kubectl/index.html |
| `docs/concepts/overview/kubernetes-api/index.html` | https://kubernetes.io/docs/concepts/overview/kubernetes-api/index.html |
| `docs/concepts/overview/working-with-objects/annotations/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/annotations/index.html |
| `docs/concepts/overview/working-with-objects/common-labels/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/common-labels/index.html |
| `docs/concepts/overview/working-with-objects/field-selectors/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/field-selectors/index.html |
| `docs/concepts/overview/working-with-objects/finalizers/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/finalizers/index.html |
| `docs/concepts/overview/working-with-objects/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/index.html |
| `docs/concepts/overview/working-with-objects/labels/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/index.html |
| `docs/concepts/overview/working-with-objects/names/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/names/index.html |
| `docs/concepts/overview/working-with-objects/namespaces/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/namespaces/index.html |
| `docs/concepts/overview/working-with-objects/object-management/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/object-management/index.html |
| `docs/concepts/overview/working-with-objects/owners-dependents/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/owners-dependents/index.html |
| `docs/concepts/overview/working-with-objects/storage-version/index.html` | https://kubernetes.io/docs/concepts/overview/working-with-objects/storage-version/index.html |
| `docs/concepts/workloads/controllers/daemonset/index.html` | https://kubernetes.io/docs/concepts/workloads/controllers/daemonset/index.html |
| `docs/concepts/workloads/controllers/deployment/index.html` | https://kubernetes.io/docs/concepts/workloads/controllers/deployment/index.html |
| `docs/concepts/workloads/controllers/index.html` | https://kubernetes.io/docs/concepts/workloads/controllers/index.html |
| `docs/concepts/workloads/controllers/replicaset/index.html` | https://kubernetes.io/docs/concepts/workloads/controllers/replicaset/index.html |
| `docs/concepts/workloads/controllers/statefulset/index.html` | https://kubernetes.io/docs/concepts/workloads/controllers/statefulset/index.html |
| `docs/concepts/workloads/index.html` | https://kubernetes.io/docs/concepts/workloads/index.html |
| `docs/concepts/workloads/pods/advanced-pod-config/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/advanced-pod-config/index.html |
| `docs/concepts/workloads/pods/disruptions/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/disruptions/index.html |
| `docs/concepts/workloads/pods/downward-api/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/downward-api/index.html |
| `docs/concepts/workloads/pods/ephemeral-containers/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/ephemeral-containers/index.html |
| `docs/concepts/workloads/pods/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/index.html |
| `docs/concepts/workloads/pods/init-containers/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/init-containers/index.html |
| `docs/concepts/workloads/pods/pod-condition/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/pod-condition/index.html |
| `docs/concepts/workloads/pods/pod-hostname/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/pod-hostname/index.html |
| `docs/concepts/workloads/pods/pod-lifecycle/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/index.html |
| `docs/concepts/workloads/pods/pod-qos/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/pod-qos/index.html |
| `docs/concepts/workloads/pods/probes/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/probes/index.html |
| `docs/concepts/workloads/pods/scheduling-group/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/scheduling-group/index.html |
| `docs/concepts/workloads/pods/sidecar-containers/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/sidecar-containers/index.html |
| `docs/concepts/workloads/pods/static-pods/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/static-pods/index.html |
| `docs/concepts/workloads/pods/user-namespaces/index.html` | https://kubernetes.io/docs/concepts/workloads/pods/user-namespaces/index.html |
| `docs/concepts/workloads/workload-api/disruption-and-priority/index.html` | https://kubernetes.io/docs/concepts/workloads/workload-api/disruption-and-priority/index.html |
| `docs/concepts/workloads/workload-api/index.html` | https://kubernetes.io/docs/concepts/workloads/workload-api/index.html |
| `docs/concepts/workloads/workload-api/policies/index.html` | https://kubernetes.io/docs/concepts/workloads/workload-api/policies/index.html |
| `docs/concepts/workloads/workload-api/topology-aware-scheduling/index.html` | https://kubernetes.io/docs/concepts/workloads/workload-api/topology-aware-scheduling/index.html |

### `node` — nodejs.org/api (42 pages)

| Path | URL |
|---|---|
| `api/addons.html` | https://nodejs.org/api/addons.html |
| `api/assert.html` | https://nodejs.org/api/assert.html |
| `api/async_context.html` | https://nodejs.org/api/async_context.html |
| `api/async_hooks.html` | https://nodejs.org/api/async_hooks.html |
| `api/buffer.html` | https://nodejs.org/api/buffer.html |
| `api/child_process.html` | https://nodejs.org/api/child_process.html |
| `api/cli.html` | https://nodejs.org/api/cli.html |
| `api/cluster.html` | https://nodejs.org/api/cluster.html |
| `api/console.html` | https://nodejs.org/api/console.html |
| `api/crypto.html` | https://nodejs.org/api/crypto.html |
| `api/debugger.html` | https://nodejs.org/api/debugger.html |
| `api/deprecations.html` | https://nodejs.org/api/deprecations.html |
| `api/diagnostics_channel.html` | https://nodejs.org/api/diagnostics_channel.html |
| `api/dns.html` | https://nodejs.org/api/dns.html |
| `api/documentation.html` | https://nodejs.org/api/documentation.html |
| `api/domain.html` | https://nodejs.org/api/domain.html |
| `api/embedding.html` | https://nodejs.org/api/embedding.html |
| `api/environment_variables.html` | https://nodejs.org/api/environment_variables.html |
| `api/errors.html` | https://nodejs.org/api/errors.html |
| `api/esm.html` | https://nodejs.org/api/esm.html |
| `api/events.html` | https://nodejs.org/api/events.html |
| `api/ffi.html` | https://nodejs.org/api/ffi.html |
| `api/fs.html` | https://nodejs.org/api/fs.html |
| `api/globals.html` | https://nodejs.org/api/globals.html |
| `api/http.html` | https://nodejs.org/api/http.html |
| `api/http2.html` | https://nodejs.org/api/http2.html |
| `api/https.html` | https://nodejs.org/api/https.html |
| `api/index.html` | https://nodejs.org/api/index.html |
| `api/inspector.html` | https://nodejs.org/api/inspector.html |
| `api/intl.html` | https://nodejs.org/api/intl.html |
| `api/module.html` | https://nodejs.org/api/module.html |
| `api/modules.html` | https://nodejs.org/api/modules.html |
| `api/n-api.html` | https://nodejs.org/api/n-api.html |
| `api/net.html` | https://nodejs.org/api/net.html |
| `api/os.html` | https://nodejs.org/api/os.html |
| `api/packages.html` | https://nodejs.org/api/packages.html |
| `api/path.html` | https://nodejs.org/api/path.html |
| `api/querystring.html` | https://nodejs.org/api/querystring.html |
| `api/stream_iter.html` | https://nodejs.org/api/stream_iter.html |
| `api/synopsis.html` | https://nodejs.org/api/synopsis.html |
| `api/typescript.html` | https://nodejs.org/api/typescript.html |
| `api/url.html` | https://nodejs.org/api/url.html |

### `python` — docs.python.org (82 pages)

| Path | URL |
|---|---|
| `3/library/allos.html` | https://docs.python.org/3/library/allos.html |
| `3/library/array.html` | https://docs.python.org/3/library/array.html |
| `3/library/binary.html` | https://docs.python.org/3/library/binary.html |
| `3/library/bisect.html` | https://docs.python.org/3/library/bisect.html |
| `3/library/calendar.html` | https://docs.python.org/3/library/calendar.html |
| `3/library/cmath.html` | https://docs.python.org/3/library/cmath.html |
| `3/library/codecs.html` | https://docs.python.org/3/library/codecs.html |
| `3/library/collections.abc.html` | https://docs.python.org/3/library/collections.abc.html |
| `3/library/collections.html` | https://docs.python.org/3/library/collections.html |
| `3/library/constants.html` | https://docs.python.org/3/library/constants.html |
| `3/library/copy.html` | https://docs.python.org/3/library/copy.html |
| `3/library/datatypes.html` | https://docs.python.org/3/library/datatypes.html |
| `3/library/datetime.html` | https://docs.python.org/3/library/datetime.html |
| `3/library/decimal.html` | https://docs.python.org/3/library/decimal.html |
| `3/library/difflib.html` | https://docs.python.org/3/library/difflib.html |
| `3/library/email.iterators.html` | https://docs.python.org/3/library/email.iterators.html |
| `3/library/enum.html` | https://docs.python.org/3/library/enum.html |
| `3/library/errno.html` | https://docs.python.org/3/library/errno.html |
| `3/library/exceptions.html` | https://docs.python.org/3/library/exceptions.html |
| `3/library/fileinput.html` | https://docs.python.org/3/library/fileinput.html |
| `3/library/fractions.html` | https://docs.python.org/3/library/fractions.html |
| `3/library/functions.html` | https://docs.python.org/3/library/functions.html |
| `3/library/getpass.html` | https://docs.python.org/3/library/getpass.html |
| `3/library/graphlib.html` | https://docs.python.org/3/library/graphlib.html |
| `3/library/heapq.html` | https://docs.python.org/3/library/heapq.html |
| `3/library/index.html` | https://docs.python.org/3/library/index.html |
| `3/library/intro.html` | https://docs.python.org/3/library/intro.html |
| `3/library/io.html` | https://docs.python.org/3/library/io.html |
| `3/library/json.html` | https://docs.python.org/3/library/json.html |
| `3/library/locale.html` | https://docs.python.org/3/library/locale.html |
| `3/library/mailbox.html` | https://docs.python.org/3/library/mailbox.html |
| `3/library/marshal.html` | https://docs.python.org/3/library/marshal.html |
| `3/library/math.html` | https://docs.python.org/3/library/math.html |
| `3/library/netdata.html` | https://docs.python.org/3/library/netdata.html |
| `3/library/numbers.html` | https://docs.python.org/3/library/numbers.html |
| `3/library/numeric.html` | https://docs.python.org/3/library/numeric.html |
| `3/library/os.html` | https://docs.python.org/3/library/os.html |
| `3/library/os.path.html` | https://docs.python.org/3/library/os.path.html |
| `3/library/pathlib.html` | https://docs.python.org/3/library/pathlib.html |
| `3/library/pickle.html` | https://docs.python.org/3/library/pickle.html |
| `3/library/platform.html` | https://docs.python.org/3/library/platform.html |
| `3/library/pprint.html` | https://docs.python.org/3/library/pprint.html |
| `3/library/pty.html` | https://docs.python.org/3/library/pty.html |
| `3/library/random.html` | https://docs.python.org/3/library/random.html |
| `3/library/re.html` | https://docs.python.org/3/library/re.html |
| `3/library/readline.html` | https://docs.python.org/3/library/readline.html |
| `3/library/reprlib.html` | https://docs.python.org/3/library/reprlib.html |
| `3/library/rlcompleter.html` | https://docs.python.org/3/library/rlcompleter.html |
| `3/library/shutil.html` | https://docs.python.org/3/library/shutil.html |
| `3/library/socket.html` | https://docs.python.org/3/library/socket.html |
| `3/library/stdtypes.html` | https://docs.python.org/3/library/stdtypes.html |
| `3/library/string.html` | https://docs.python.org/3/library/string.html |
| `3/library/string.templatelib.html` | https://docs.python.org/3/library/string.templatelib.html |
| `3/library/stringprep.html` | https://docs.python.org/3/library/stringprep.html |
| `3/library/struct.html` | https://docs.python.org/3/library/struct.html |
| `3/library/sys.html` | https://docs.python.org/3/library/sys.html |
| `3/library/sysconfig.html` | https://docs.python.org/3/library/sysconfig.html |
| `3/library/tempfile.html` | https://docs.python.org/3/library/tempfile.html |
| `3/library/text.html` | https://docs.python.org/3/library/text.html |
| `3/library/textwrap.html` | https://docs.python.org/3/library/textwrap.html |
| `3/library/threadsafety.html` | https://docs.python.org/3/library/threadsafety.html |
| `3/library/types.html` | https://docs.python.org/3/library/types.html |
| `3/library/unicodedata.html` | https://docs.python.org/3/library/unicodedata.html |
| `3/library/weakref.html` | https://docs.python.org/3/library/weakref.html |
| `3/library/zoneinfo.html` | https://docs.python.org/3/library/zoneinfo.html |
| `3/tutorial/appendix.html` | https://docs.python.org/3/tutorial/appendix.html |
| `3/tutorial/appetite.html` | https://docs.python.org/3/tutorial/appetite.html |
| `3/tutorial/classes.html` | https://docs.python.org/3/tutorial/classes.html |
| `3/tutorial/controlflow.html` | https://docs.python.org/3/tutorial/controlflow.html |
| `3/tutorial/datastructures.html` | https://docs.python.org/3/tutorial/datastructures.html |
| `3/tutorial/errors.html` | https://docs.python.org/3/tutorial/errors.html |
| `3/tutorial/floatingpoint.html` | https://docs.python.org/3/tutorial/floatingpoint.html |
| `3/tutorial/index.html` | https://docs.python.org/3/tutorial/index.html |
| `3/tutorial/inputoutput.html` | https://docs.python.org/3/tutorial/inputoutput.html |
| `3/tutorial/interactive.html` | https://docs.python.org/3/tutorial/interactive.html |
| `3/tutorial/interpreter.html` | https://docs.python.org/3/tutorial/interpreter.html |
| `3/tutorial/introduction.html` | https://docs.python.org/3/tutorial/introduction.html |
| `3/tutorial/modules.html` | https://docs.python.org/3/tutorial/modules.html |
| `3/tutorial/stdlib.html` | https://docs.python.org/3/tutorial/stdlib.html |
| `3/tutorial/stdlib2.html` | https://docs.python.org/3/tutorial/stdlib2.html |
| `3/tutorial/venv.html` | https://docs.python.org/3/tutorial/venv.html |
| `3/tutorial/whatnow.html` | https://docs.python.org/3/tutorial/whatnow.html |

### `rust-std` — doc.rust-lang.org/std (84 pages)

| Path | URL |
|---|---|
| `std/all.html` | https://doc.rust-lang.org/std/all.html |
| `std/boxed/index.html` | https://doc.rust-lang.org/std/boxed/index.html |
| `std/cell/struct.Cell.html` | https://doc.rust-lang.org/std/cell/struct.Cell.html |
| `std/cell/struct.RefCell.html` | https://doc.rust-lang.org/std/cell/struct.RefCell.html |
| `std/char/index.html` | https://doc.rust-lang.org/std/char/index.html |
| `std/cmp/index.html` | https://doc.rust-lang.org/std/cmp/index.html |
| `std/collections/index.html` | https://doc.rust-lang.org/std/collections/index.html |
| `std/collections/struct.HashMap.html` | https://doc.rust-lang.org/std/collections/struct.HashMap.html |
| `std/env/index.html` | https://doc.rust-lang.org/std/env/index.html |
| `std/fs/enum.TryLockError.html` | https://doc.rust-lang.org/std/fs/enum.TryLockError.html |
| `std/fs/fn.canonicalize.html` | https://doc.rust-lang.org/std/fs/fn.canonicalize.html |
| `std/fs/fn.copy.html` | https://doc.rust-lang.org/std/fs/fn.copy.html |
| `std/fs/fn.create_dir.html` | https://doc.rust-lang.org/std/fs/fn.create_dir.html |
| `std/fs/fn.create_dir_all.html` | https://doc.rust-lang.org/std/fs/fn.create_dir_all.html |
| `std/fs/fn.exists.html` | https://doc.rust-lang.org/std/fs/fn.exists.html |
| `std/fs/fn.hard_link.html` | https://doc.rust-lang.org/std/fs/fn.hard_link.html |
| `std/fs/fn.metadata.html` | https://doc.rust-lang.org/std/fs/fn.metadata.html |
| `std/fs/fn.read.html` | https://doc.rust-lang.org/std/fs/fn.read.html |
| `std/fs/fn.read_dir.html` | https://doc.rust-lang.org/std/fs/fn.read_dir.html |
| `std/fs/fn.read_link.html` | https://doc.rust-lang.org/std/fs/fn.read_link.html |
| `std/fs/fn.read_to_string.html` | https://doc.rust-lang.org/std/fs/fn.read_to_string.html |
| `std/fs/fn.remove_dir.html` | https://doc.rust-lang.org/std/fs/fn.remove_dir.html |
| `std/fs/fn.remove_dir_all.html` | https://doc.rust-lang.org/std/fs/fn.remove_dir_all.html |
| `std/fs/fn.remove_file.html` | https://doc.rust-lang.org/std/fs/fn.remove_file.html |
| `std/fs/fn.rename.html` | https://doc.rust-lang.org/std/fs/fn.rename.html |
| `std/fs/fn.set_permissions.html` | https://doc.rust-lang.org/std/fs/fn.set_permissions.html |
| `std/fs/fn.set_permissions_nofollow.html` | https://doc.rust-lang.org/std/fs/fn.set_permissions_nofollow.html |
| `std/fs/fn.set_times.html` | https://doc.rust-lang.org/std/fs/fn.set_times.html |
| `std/fs/fn.set_times_nofollow.html` | https://doc.rust-lang.org/std/fs/fn.set_times_nofollow.html |
| `std/fs/fn.soft_link.html` | https://doc.rust-lang.org/std/fs/fn.soft_link.html |
| `std/fs/fn.symlink_metadata.html` | https://doc.rust-lang.org/std/fs/fn.symlink_metadata.html |
| `std/fs/fn.write.html` | https://doc.rust-lang.org/std/fs/fn.write.html |
| `std/fs/index.html` | https://doc.rust-lang.org/std/fs/index.html |
| `std/fs/struct.Dir.html` | https://doc.rust-lang.org/std/fs/struct.Dir.html |
| `std/fs/struct.DirBuilder.html` | https://doc.rust-lang.org/std/fs/struct.DirBuilder.html |
| `std/fs/struct.DirEntry.html` | https://doc.rust-lang.org/std/fs/struct.DirEntry.html |
| `std/fs/struct.File.html` | https://doc.rust-lang.org/std/fs/struct.File.html |
| `std/fs/struct.FileTimes.html` | https://doc.rust-lang.org/std/fs/struct.FileTimes.html |
| `std/fs/struct.FileType.html` | https://doc.rust-lang.org/std/fs/struct.FileType.html |
| `std/fs/struct.Metadata.html` | https://doc.rust-lang.org/std/fs/struct.Metadata.html |
| `std/fs/struct.OpenOptions.html` | https://doc.rust-lang.org/std/fs/struct.OpenOptions.html |
| `std/fs/struct.Permissions.html` | https://doc.rust-lang.org/std/fs/struct.Permissions.html |
| `std/fs/struct.ReadDir.html` | https://doc.rust-lang.org/std/fs/struct.ReadDir.html |
| `std/index.html` | https://doc.rust-lang.org/std/index.html |
| `std/io/index.html` | https://doc.rust-lang.org/std/io/index.html |
| `std/iter/index.html` | https://doc.rust-lang.org/std/iter/index.html |
| `std/iter/trait.Iterator.html` | https://doc.rust-lang.org/std/iter/trait.Iterator.html |
| `std/macro.format.html` | https://doc.rust-lang.org/std/macro.format.html |
| `std/net/index.html` | https://doc.rust-lang.org/std/net/index.html |
| `std/net/struct.TcpStream.html` | https://doc.rust-lang.org/std/net/struct.TcpStream.html |
| `std/net/struct.UdpSocket.html` | https://doc.rust-lang.org/std/net/struct.UdpSocket.html |
| `std/option/enum.Option.html` | https://doc.rust-lang.org/std/option/enum.Option.html |
| `std/option/index.html` | https://doc.rust-lang.org/std/option/index.html |
| `std/prelude/index.html` | https://doc.rust-lang.org/std/prelude/index.html |
| `std/primitive.array.html` | https://doc.rust-lang.org/std/primitive.array.html |
| `std/primitive.char.html` | https://doc.rust-lang.org/std/primitive.char.html |
| `std/primitive.slice.html` | https://doc.rust-lang.org/std/primitive.slice.html |
| `std/primitive.str.html` | https://doc.rust-lang.org/std/primitive.str.html |
| `std/rc/struct.Rc.html` | https://doc.rust-lang.org/std/rc/struct.Rc.html |
| `std/result/enum.Result.html` | https://doc.rust-lang.org/std/result/enum.Result.html |
| `std/result/index.html` | https://doc.rust-lang.org/std/result/index.html |
| `std/slice/index.html` | https://doc.rust-lang.org/std/slice/index.html |
| `std/str/trait.FromStr.html` | https://doc.rust-lang.org/std/str/trait.FromStr.html |
| `std/string/index.html` | https://doc.rust-lang.org/std/string/index.html |
| `std/string/struct.String.html` | https://doc.rust-lang.org/std/string/struct.String.html |
| `std/sync/atomic/fn.compiler_fence.html` | https://doc.rust-lang.org/std/sync/atomic/fn.compiler_fence.html |
| `std/sync/atomic/fn.fence.html` | https://doc.rust-lang.org/std/sync/atomic/fn.fence.html |
| `std/sync/atomic/index.html` | https://doc.rust-lang.org/std/sync/atomic/index.html |
| `std/sync/index.html` | https://doc.rust-lang.org/std/sync/index.html |
| `std/sync/mpmc/index.html` | https://doc.rust-lang.org/std/sync/mpmc/index.html |
| `std/sync/mpsc/index.html` | https://doc.rust-lang.org/std/sync/mpsc/index.html |
| `std/sync/struct.Arc.html` | https://doc.rust-lang.org/std/sync/struct.Arc.html |
| `std/sync/struct.Mutex.html` | https://doc.rust-lang.org/std/sync/struct.Mutex.html |
| `std/sync/struct.Once.html` | https://doc.rust-lang.org/std/sync/struct.Once.html |
| `std/sync/struct.OnceLock.html` | https://doc.rust-lang.org/std/sync/struct.OnceLock.html |
| `std/sync/struct.RwLock.html` | https://doc.rust-lang.org/std/sync/struct.RwLock.html |
| `std/thread/index.html` | https://doc.rust-lang.org/std/thread/index.html |
| `std/vec/index.html` | https://doc.rust-lang.org/std/vec/index.html |
| `std/vec/struct.Drain.html` | https://doc.rust-lang.org/std/vec/struct.Drain.html |
| `std/vec/struct.ExtractIf.html` | https://doc.rust-lang.org/std/vec/struct.ExtractIf.html |
| `std/vec/struct.IntoIter.html` | https://doc.rust-lang.org/std/vec/struct.IntoIter.html |
| `std/vec/struct.PeekMut.html` | https://doc.rust-lang.org/std/vec/struct.PeekMut.html |
| `std/vec/struct.Splice.html` | https://doc.rust-lang.org/std/vec/struct.Splice.html |
| `std/vec/struct.Vec.html` | https://doc.rust-lang.org/std/vec/struct.Vec.html |
