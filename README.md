# (bi)bin

A fork of the excellent [bin](https://github.com/w4/bin). I use that project on my own website as a quick scratchpad, url shortener, QR code generator... I also added a rudimentary password protection scheme because I did not want to have to curate the content.

---

A paste bin that's no longer minimalist. But it still has no database requirement, no commenting functionality, no self-destructing or time bomb messages and no social media integration-just an application to quickly send snippets of text to people.

It is written in Rust. It's fast, it's simple, there's code highlighting. It's revolutionary in the paste bin industry, disrupting markets and pushing boundaries never seen before).

##### how do you run it?

```bash
$ ROCKET_PREFIX="https://bi.bin" ROCKET_PASSWORD=bibinrulez ./bibin
```

##### funny, what settings are there?

bin uses [rocket](https://rocket.rs) so the configuration is done with a [rocket config file](https://api.rocket.rs/v0.3/rocket/config/). You can set `ROCKET_PORT` in your environment if you want to change the default port (8000).

An environment variable `BIN_BUFFER_SIZE` (which defaults to 2000) define how many paste are stored.

You will need to provide the url prefix that will be used to generate the URL in the QR codes (`PREFIX`) as well as the password (`PASSWORD`) in the rocket config file.

##### is there curl support?

```bash
$ curl -X PUT -u "anything:PASSWORD" --data 'hello world' https://bi.bin/
# or
$ curl -X PUT -H "X-API-Key:PASSWORD" --data 'hello world' https://bi.bin/
https://bi.bin/cateettary
$ curl https://bi.bin/cateettary
hello world
```

##### What can bibin do?

**Scratchpad**: Everything that you write will be stored on your browser, so you can close the window and what you typed will be there again when you come back.

**Syntax highlighting**: you need to add the file extension at the end of your paste URL.

**Special extensions**:
- `.url` will trigger a http redirect to whatever is in the post. This works with curl requests as well!
- `.b64` will return the content base64-encoded
- `.qr` will return the content as a qr code

**Generate a QR code for your url**: just add /qr at the end of your bibin URL: `https://bi.bin/cateettary.c/qr`
