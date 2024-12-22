# Client Installation & Configuration

## Download mieru client

The mieru client supports Windows, Mac OS, and Linux. Users can download it from the GitHub Releases page. After unzip, place the mieru executable under the system path `PATH`.

If your client OS is Linux, you can also install mieru using the debian and RPM installers.

## Modify proxy client settings

Use can invoke command

```sh
mieru apply config <FILE>
```

to modify the proxy client settings. `<FILE>` is a JSON formatted configuration file. An example of client configuration is as follows.

```js
{
    "profiles": [
        {
            "profileName": "default",
            "user": {
                "name": "ducaiguozei",
                "password": "xijinping"
            },
            "servers": [
                {
                    "ipAddress": "12.34.56.78",
                    "domainName": "",
                    "portBindings": [
                        {
                            "portRange": "2012-2022",
                            "protocol": "TCP"
                        },
                        {
                            "port": 2027,
                            "protocol": "TCP"
                        }
                    ]
                }
            ],
            "mtu": 1400,
            "multiplexing": {
                "level": "MULTIPLEXING_HIGH"
            }
        }
    ],
    "activeProfile": "default",
    "rpcPort": 8964,
    "socks5Port": 1080,
    "loggingLevel": "INFO",
    "socks5ListenLAN": false,
    "httpProxyPort": 8080,
    "httpProxyListenLAN": false
}
```

Please use a text editor to modify the following fields.

1. In the `profiles` -> `user` -> `name` property, fill in the username. This must be the same as the setting in the proxy server.
2. In the `profiles` -> `user` -> `password` property, fill in the password. This must be the same as the setting in the proxy server.
3. In the `profiles` -> `servers` -> `ipAddress` property, fill in the public address of the proxy server. Both IPv4 and IPv6 addresses are supported.
4. If you have registered a domain name for the proxy server, please fill in the domain name in `profiles` -> `servers` -> `domainName`. Otherwise, do not modify this property.
5. Fill in `profiles` -> `servers` -> `portBindings` -> `port` with the TCP or UDP port number that mita is listening to. The port number must be the same as the one set in the proxy server. If you want to listen to a range of consecutive port numbers, you can also use the `portRange` property instead.
6. Specify a value between 1280 and 1400 for the `profiles` -> `mtu` property. The default value is 1400. This value can be different from the setting in the proxy server.
7. If you want to adjust the frequency of multiplexing, you can set a value for the `profiles` -> `multiplexing` -> `level` property. The values you can use here include `MULTIPLEXING_OFF`, `MULTIPLEXING_LOW`, `MULTIPLEXING_MIDDLE`, and `MULTIPLEXING_HIGH`. `MULTIPLEXING_OFF` will disable multiplexing, and the default value is `MULTIPLEXING_LOW`.
8. Please specify a value between 1025 and 65535 for the `rpcPort` property.
9. Please specify a value between 1025 and 65535 for the `socks5Port` property. This port cannot be the same as `rpcPort`.
10. If the client needs to provide proxy services to other devices on the LAN, set the `socks5ListenLAN` property to `true`.
11. If you want to enable HTTP / HTTPS proxy, Please specify a value between 1025 and 65535 for the `httpProxyPort` property. This port cannot be the same as `rpcPort` or `socks5Port`. If the client needs to provide HTTP / HTTPS proxy services to other devices on the LAN, set the `httpProxyListenLAN` property to `true`. If you want to disable HTTP / HTTPS proxy, please delete `httpProxyPort` and `httpProxyListenLAN` property.

If you have multiple proxy servers installed, or one server listening on multiple ports, you can add them all to the client settings. Each time a new connection is created, mieru will randomly select one of the servers and one of the ports. **If you are using multiple servers, make sure that each server has the mita proxy service started.**

Assuming the file name of this configuration file is `client_config.json`, call command `mieru apply config client_config.json` to write the configuration after it has been modified.

If the configuration is incorrect, mieru will print the problem that occurred. Follow the prompts to modify the configuration file and re-run the `mieru apply config <FILE>` command to write the configuration.

After that, invoke command

```sh
mieru describe config
```

to check the current proxy settings.

## Start proxy client

```sh
mieru start
```

If the output shows `mieru client is started, listening to xxxxx`, it means that the mieru client is running in the background.

The mieru client will not be started automatically with system boot. After restarting the computer, you need to start the client manually with the `mieru start` command.

**Windows users should note that after starting the client with the `mieru start` command at the command prompt or Powershell, do not close the command prompt or Powershell window. Closing the window will cause the mieru client to exit.** Some new versions of Windows allow users to minimize the command prompt or Powershell to the tray.

If you need to stop the mieru client, enter the following command

```sh
mieru stop
```

Note that every time you change the settings with `mieru apply config <FILE>`, you need to restart the client with `mieru stop` and `mieru start` for the new settings to take effect.

## Test the Connection Between Client and Server

```sh
mieru test

OR

mieru test https://<website.you.want.to.connect>
```

If the output shows `Connected to ...`, it indicates that the mieru client has successfully connected to the proxy server.

## Configuring the browser

Chrome / Firefox and other browsers can use socks5 proxy to access blocked websites by installing browser plugins. For the address of the socks5 proxy, please fill in `127.0.0.1:xxxx`, where `xxxx` is the value of `socks5Port` in the client settings. This address will also be printed when the `mieru start` command is called.

mieru doesn't use socks5 authentication.

For configuring the socks5 proxy in the Tor browser, see the [Security Guide](./security.md).

## Advanced Settings

### socks5 Username and Password Authentication

If you want to require applications to authenticate the socks5 proxy using a username and password, you can add the `socks5Authentication` property to the client configuration. An example is as follows:

```js
{
    "socks5Authentication": [
        {
            "user": "yitukai",
            "password": "manlianpenfen"
        },
        {
            "user": "shilishanlu",
            "password": "buhuanjian"
        }
    ]
}
```

Applications can choose any user and password in the `socks5Authentication` list to authenticate the socks5 proxy.

**socks5 username and password authentication is not compatible with HTTP / HTTPS proxy.** Since HTTP / HTTPS proxy does not require username and password authentication, based on threat model, mieru prohibits the use of HTTP / HTTPS proxy in conjunction with socks5 username and password authentication.

If you need to delete an existing HTTP / HTTPS proxy configuration, please run the `mieru delete http proxy` command. If you want to delete the socks5 username and password authentication settings, please run the `mieru delete socks5 authentication` command.
