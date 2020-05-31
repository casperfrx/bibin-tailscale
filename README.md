# bibin

A fork of the excellent [bin](https://github.com/w4/bin). I really didn't change much, even this README. I implemented (hacked?) password protection and qr generation. To get the QR code, just append `/qr` at the
end of the url (`https://bi.bin/cateettary.md/qr`). In order to publish a new bin, you will need to provide a password. It can also be used as a URL shortener.

---

A paste bin that's ~actually~ ~almost~ a bit minimalist. No database requirement, no commenting functionality, no self-destructing or time bomb messages and no social media integration—just an application to quickly send snippets of text to people.

It is written in Rust in around ~200~ ~280~ 300 lines of code. It's fast, it's simple, there's code highlighting ~and you can ⌘+A without going to the 'plain' page~. It's revolutionary in the paste bin industry, disrupting markets and pushing boundaries never seen before).

##### how do you run it?

```bash
$ ROCKET_prefix="https://bi.bin" ROCKET_PASSWORD=bibinrulez ./bin
```

##### funny, what settings are there?

bin uses [rocket](https://rocket.rs) so you can add a [rocket config file](https://api.rocket.rs/v0.3/rocket/config/) if you like. You can set `ROCKET_PORT` in your environment if you want to change the default port (8000).

bin's ~only~ configuration value is `BIN_BUFFER_SIZE` which defaults to 2000. Change this value if you want your bin to hold more pastes.

You will need to provide the url prefix that will be used to generate the URL in the QR codes as well as the password in the rocket config file.

##### is there curl support?

```bash
$ curl -X PUT --data 'hello world' https://bi.bin/PASSWORD
https://bi.bin/cateettary
$ curl https://bi.bin/cateettary
hello world
```

##### how does syntax highlighting and URL shortening work?

To get syntax highlighting you need to add the file extension at the end of your paste URL. The `.url` special extension will trigger a http redirect to whatever is in the post. This works with curl requests as well!
