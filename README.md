# VectorBackup
`VectorBackup` is a system for automatic periodic backup of the most important resources that `VectorCircles` organization has.

As of now, the program supports the following sources:
- Google Drive

It is also planned to support these sources in the future:
- Trello

## Building from source
In order to install the program from source, one is ought to install `Rust` programming language toolchain. One of the recommended installation options is described [here](https://www.rust-lang.org/tools/install).

When the `Rust` toolchain is installed, one should run the following commands (assuming `*nix`, but `Windows` commands have their own alternatives) to build the program.

```bash
git clone https://git.spcraft.ga/VectorCircles/AutoBackup.git

cd AutoBackup

cargo build --release
```

After some time, the executable may be found at `./target/release/vectorcircles-auto-backup` (or some `.exe` file in case of `Windows`), assuming `AutoBackup` to be the current directory.

## Basic Usage
To use the program, one should copy the executable to some folder, where the backups are going to be stored, and then run it.

During the first run, the program will generate the `config.yml` file, two fileds of which have to be filled up.

```yaml
# Unimportant fields omitted
google_drive:
  client_id: my_awesome_client # <-- Fill this field up
  client_secret: my_awesom_secret # <-- This one as well
```

You need to acquire some google API credentials from the author of this application, or register your own.

In the end, your configuration file will look something like:

```yaml
# Unimportant fields omitted
google_drive:
  client_id: 313540835121-ljnf6fbqkhqmd263rrpgtmog9jpr763n.apps.googleusercontent.com
  client_secret: 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G
```

When the configuration is set, the execution of the binary will print a URL to the stdout, which should be opened in your browser.

The link will open a dialog window, which will be asking for the google account authorization. Upon successful log in, one will be asked, whether it wishes to allow the application to acquire certain permissions, namely:

- See and Download all Drive files
- See info about the Drive files

The program will not work properly, unless both permissions are given. As soon as the permissions are granted, the backup process will commence.
