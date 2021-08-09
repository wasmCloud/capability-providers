# Configuration settings for httpserver

Configuration for httpserver can be provided in either json or toml format. Json examples are used below.

## Fields and default values

Anything not specified in your settings file inherits the default.

### Address

Address is a string in the form "IP:PORT". The default bind address is "127.0.0.1:8000". The IP address may be an IPV4 or IPV6 address.

### TLS

An empty tls section, or no tls section, disables tls. To enable tls, both `cert_file` and `priv_key_file` must contain absolute paths to existing files.

### CORS

- `allowed_origins` - a list of allowed origin addresses. See [`Access-Control-Allow-Origin`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin) Each origin must begin with either 'http:' or 'https:'. If the list is empty (the default) all origins are allowed. The default setting allows all origin hosts.
  
- `allowed_methods` - a list of upper case http methods. See [`Access-Control-Allow-Headers`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Methods) Default:

  ```json
  ["GET","POST","PUT","DELETE","HEAD","OPTIONS"]
  ```
  
- `allowed_headers` - a list of allowed headers, case-insensitive. See [`Access-Control-Allow-Headers`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Headers) Default:

  ```json
  ["accept", "accept-language", "content-type", "content-language"]
  ```

- `exposed_headers` - see [`Access-Control-Expose-Headers`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Expose-Headers)

- `max_age_secs` - sets the `Access-Control-Max-Age` header. Default is 300 seconds.


## Examples of settings files

Bind to all IP interfaces and port 3000, no TLS

```json
{ "address":  "0.0.0.0:3000" }
```

Example with all settings

```json
{
  "address": "127.0.0.1:8000",
  "tls": {
    "cert_file": "/path/to/certificate.crt",
    "priv_key_file": "/path/to/private.key"
  },
  "cors": {
    "allowed_origins": [],
    "allowed_headers": [
      "accept", "accept-language", "content-language", "content-type", "x-custome-header"
    ],
    "allowed_methods": [ "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS" ],
    "exposed_headers": [],
    "max_age_secs": 300
  }
}
```

## Using the http server settings

The link definition connecting an actor to a capability provider includes a key-value map called "values". (The location of this 'values' map is still tbd, possibly in a manifest file). "values" is currently defined as a map with string keys and string values, and there are a few options for specifying the httpserver settings within the values map.

- use key `config_file`, with a value that is the absolute path to a json or toml file (file type detected by the `.json` or `.toml` extension). 

- use key `config_b64`, with a value that contains the settings base64-encoded. If you have the base64 utility, this command can generate the string value:
  ```sh
  cat settings.json | base64 -w0
  ```
  (The option `-w0` omits line breaks within the base64)

- use key `config_json`, with a value that is the raw json. Don't forget to escape quotes.