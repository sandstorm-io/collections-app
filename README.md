# Sandstorm Collections App

This is an app that runs on [Sandstorm](https://sandstorm.io).
Its purpose is to aggregate a group of
[grains](https://docs.sandstorm.io/en/latest/using/security-practices/#fine-grained-isolation)
so that they can be shared as a single unit.

You can install it from the Sandstorm App Market
[here](https://apps.sandstorm.io/app/s3u2xgmqwznz2n3apf30sm3gw1d85y029enw5pymx734cnk5n78h).

## Developing

You will need:
  - A recent build of [Cap'n Proto](https://github.com/sandstorm-io/capnproto) from the master branch,
    installed such that the `capnp` executable is on your PATH.
  - A [dev install of Sandstorm](https://docs.sandstorm.io/en/latest/developing/raw-packaging-guide/)
  - [Rust](https://rust-lang.org)
  - [Node](https://nodejs.org) and [NPM](https://www.npmjs.com/)


```
$ npm install
$ make dev
```