# Nyanapsu Service

A Service for Nyanapsu to make it easier to operate the privileged actions.

## Relations

![relation](./.github/assets/nyanpasu-service.drawio.svg)

This project includes two crates:

* `nyanpasu-ipc` a ipc bridge crate between the service and the client. It provide a `create_server` fn to hold a axum server, and provide a `shortcuts` mod for swift client rpc call.
  * It use `named_pipe` in windows, and `unix_socket` in unix-like system.
  * When install service, it should collect the users info (sid in windows, username in unix) for security.
    * Grant ACL to the pipe
    * When installing, add user to `nyanpasu` group, and grant the group to the pipe.
* `nyanpasu-service` it the main entrance of the service, it provide a control plane to manage the service, and provide a `rpc` subcommand to test the service.


## Development

Run with development preference:

```shell
cargo debug-run
```

Build with development preference:

```shell
cargo debug-build
```

View the service info:

```shell
./nyanpasu-service status # service status and health check(if running)
./nyanpasu-service version # build info only
```
