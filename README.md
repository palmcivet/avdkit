# avdkit

Wraps Android virtual device (AVD) management tools as a programmable interface: a Rust core, with language bindings and a command-line interface that emits JSON.

Compatible with Android CLI (`android`) and `cmdline-tools`. Callers can query the local machine’s capabilities before invoking a tool. Operations that no single tool can complete are expressed as multi-step plans that share one preflight, execution, and compensation path.
