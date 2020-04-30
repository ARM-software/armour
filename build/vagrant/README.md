# rust-build

- Install vagrant, e.g.

```
$ brew cask install vagrant
```

- Install the docker compose plug-in

```
$ vagrant plugin install vagrant-docker-compose
```

- Create a Vagrant VM called `rust-build` for rust cross-compilation

```shell
$ sh setup.sh $TARGET_DIR
```

- Connect

```
$ vagrant ssh
```

- Suspend

```
$ vagrant suspend
```

- Resume

```
$ vagrant resume
```

- Remove

```
$ vagrant destroy
```
