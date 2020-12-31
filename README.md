# (bi)bin

A fork of the excellent [bin](https://github.com/w4/bin). It is a pastebin-like project for my own website: quick scratchpad, url shortener, QR code generator... I also added a rudimentary password protection scheme because I did not want to have to curate the content.

It is optimized for speed and can handle hundreds of clients per second (async code with [Rocket](https://rocket.rs/)/[SQLx](https://github.com/launchbadge/sqlx)).

### Persistence

The entries are stored in a Sqlite database. The entries are stored in the file described in the `database_file` configuration key. To persistance and use an in-memory keystore, use the special file `:memory:`.

### Configuration

(bi)bin is using [Rocket](https://rocket.rs/)'s configuration subsystem.
At startup it will read each key from a `Rocket.toml` file, or from environment variables (`ROCKET_` prefix)

```
[default]
password = "YOUR_PASSWORD"
prefix = "https://YOUR.WEBSITE.net"
secret_key = "REPLACE WITH THE OUTPUT OF openssl rand -base64 32"
```

Optional entries:
```
address = "127.0.0.1"
port = "8000"
id_length = 4   # Size of the unique paste ID
max_entries = 10000   # Maximum number of paste kept in the database.
database_connections = 10    # Number of read-only connections to the DB opened in parallel
database_file=":memory:"    # Sqlite file on disk or ":memory:"
```

Override values from `Rocket.toml` with environment variables:
```
$ ROCKET_PREFIX="https://bi.bin" ROCKET_PASSWORD=bibinrulez ROCKET_ID_LENGTH=6 ./bibin
```

### Curl support

(bi)bin support the `Basic` authentication scheme (`-u` with curl), as well as the `X-API-Key` header:

```bash
# Add a new paste
$ curl -X PUT -u "anything:PASSWORD" --data 'hello world' https://bi.bin/
# returns: https://bi.bin/cateettary
# Fetch a paste
$ curl https://bi.bin/cateettary
hello world
# Delete a paste
$ curl -X DELETE -H "X-API-Key:PASSWORD" https://bi.bin/cateettary
# Upload a new paste at a given id. Would override any existing paste there.
$ curl -X PUT -u "anything:PASSWORD" --data 'hello world' https://bi.bin/manualid
```

### What can bibin do?

**Scratchpad**: Everything that you write will be stored on your browser, so you can close the window and what you typed will be there again when you come back.

**Syntax highlighting**: you need to add the file extension at the end of your paste URL.

**Special extensions**:
- `.url` will trigger a http redirect to whatever is in the post. This works with curl requests as well!
- `.b64` will return the content base64-encoded
- `.qr` will return the content as a qr code

**Generate a QR code from the url**: Add `/qr` at the end of your bibin URL: `https://bi.bin/cateettary.c/qr`
