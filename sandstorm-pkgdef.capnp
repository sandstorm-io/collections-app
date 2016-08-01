@0xd7ed440331369f59;

using Spk = import "/sandstorm/package.capnp";

const pkgdef :Spk.PackageDefinition = (
  id = "s3u2xgmqwznz2n3apf30sm3gw1d85y029enw5pymx734cnk5n78h",

  manifest = (
    appTitle = (defaultText = "Collections"),
    appVersion = 3,  # Increment this for every release.
    appMarketingVersion = (defaultText = "0.0.3"),

    actions = [
      (
        nounPhrase = (defaultText = "collection"),
        command = .myCommand
        # The command to run when starting for the first time. (".myCommand"
        # is just a constant defined at the bottom of the file.)
      )
    ],

    continueCommand = .myCommand,
    metadata = (
      # Data which is not needed specifically to execute the app, but is useful
      # for purposes like marketing and display.  These fields are documented at
      # https://docs.sandstorm.io/en/latest/developing/publishing-apps/#add-required-metadata
      # and (in deeper detail) in the sandstorm source code, in the Metadata section of
      # https://github.com/sandstorm-io/sandstorm/blob/master/src/sandstorm/package.capnp
      icons = (
        # Various icons to represent the app in various contexts.
        appGrid = (svg = embed "icons/appGrid.svg"),
        grain = (svg = embed "icons/grain.svg"),
        market = (svg = embed "icons/market.svg"),
      ),

      website = "https://sandstorm.io",
      codeUrl = "https://github.com/dwrensha/sandstorm-collections-app",
      license = (openSource = mit),
      categories = [productivity],

      author = (
        upstreamAuthor = "David Renshaw",
        contactEmail = "david@sandstorm.io",
        pgpSignature = embed "pgp-signature",
      ),

      pgpKeyring = embed "pgp-keyring",

      description = (defaultText = embed "description.md"),

      shortDescription = (defaultText = "Grain list sharing"),
      # A very short (one-to-three words) description of what the app does. For example,
      # "Document editor", or "Notetaking", or "Email client". This will be displayed under the app
      # title in the grid view in the app market.

      screenshots = [
        (width = 1096, height = 545, png = embed "screenshots/screenshot-1.png"),
      ],
      changeLog = (defaultText = embed "CHANGELOG.md"),
    ),
  ),

  sourceMap = (
    # Here we defined where to look for files to copy into your package. The
    # `spk dev` command actually figures out what files your app needs
    # automatically by running it on a FUSE filesystem. So, the mappings
    # here are only to tell it where to find files that the app wants.
    searchPath = [
      ( sourcePath = "spk" ),
      ( sourcePath = "/",    # Then search the system root directory.
        hidePaths = [ "home", "proc", "sys",
                      "etc/passwd", "etc/hosts", "etc/host.conf",
                      "etc/nsswitch.conf", "etc/resolv.conf" ]
        # You probably don't want the app pulling files from these places,
        # so we hide them. Note that /dev, /var, and /tmp are implicitly
        # hidden because Sandstorm itself provides them.
      )
    ]
  ),

  fileList = "sandstorm-files.list",
  # `spk dev` will write a list of all the files your app uses to this file.
  # You should review it later, before shipping your app.

  alwaysInclude = [],
  # Fill this list with more names of files or directories that should be
  # included in your package, even if not listed in sandstorm-files.list.
  # Use this to force-include stuff that you know you need but which may
  # not have been detected as a dependency during `spk dev`. If you list
  # a directory here, its entire contents will be included recursively.

);

const myCommand :Spk.Manifest.Command = (
  # Here we define the command used to start up your server.
  argv = ["/collections-server"],
  environ = [
    # Note that this defines the *entire* environment seen by your app.
    (key = "PATH", value = "/usr/local/bin:/usr/bin:/bin"),
    (key = "SANDSTORM", value = "1"),
    # Export SANDSTORM=1 into the environment, so that apps running within Sandstorm
    # can detect if $SANDSTORM="1" at runtime, switching UI and/or backend to use
    # the app's Sandstorm-specific integration code.
  ]
);
