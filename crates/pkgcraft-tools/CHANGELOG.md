# 0.0.15 (2024-06-21)

## Changed
- Bumped MSRV to 1.76.
- Various documentation updates for subcommands and options.
- `pk pkg metadata`: Don't use internal targets when performing full repo scans.

# 0.0.14 (2024-02-01)

## Added
- `pk cpv`: Add CPV-related support separate from `pk dep`. This provides much
  of the same support that `pk dep` provides for package dependencies, but
  instead for CPV objects (e.g. cat/pkg-1-r2 where a corresponding package
  dependency could be =cat/pkg-1-r2).

- `pk pkg showkw`: Add initial package keyword output support. This command is
  the rough precursor to a `pkgdev showkw` and eshowkw alternative that
  currently doesn't support tabular output.

- `pk pkg env`: Add initial ebuild package environment dumping support. This
  command sources targeted ebuilds and dumps their respective bash environments
  to stdout.

- `pk pkg metadata`: Add initial support for selective package metadata
  mangling currently only allowing regeneration and verification. Where `pk
  repo metadata` operates on entire ebuild repos, this command operates on
  custom restrictions such as paths or package globs (by default it uses the
  current working directory). This allows much quicker, targeted metadata
  generation when working in specific packages directories or for scripts
  targeting specific packages.

## Changed
- `pk repo metadata`: Split actions into separate subcommands so the previous
  default regen action now must be run via `pk repo metadata regen`. Cache
  cleaning and removal are supported via the `clean` and `remove` subcommands,
  respectively.

# 0.0.13 (2023-11-06)

## Fixed
- `pk repo metadata`: use proper package prefixes for failure messages

# 0.0.12 (2023-09-29)

## Changed
- `pk dep parse`: convert --eapi value during arg parsing
- `pk repo metadata`: set default target repo to the current directory
- `pk repo metadata`: add -n/--no-progress option to disable progress bar (#140)

## Fixed
- Skip loading system config files during tests.
- Fix error propagation for utilities running in parallel across process pools.

# 0.0.11 (2023-09-06)

## Added
- `pk pkg revdeps`: initial support for querying reverse dependencies

## Changed
- Bumped MSRV to 1.70.

## Fixed
- `pk repo metadata`: remove outdated cache entries

# 0.0.10 (2023-06-23)

## Added
- Support loading the config from a custom path and disabling config loading
  via the `PKGCRAFT_NO_CONFIG` environment variable (#115).

## Fixed
- `pk repo metadata`: ignore `declare` errors with unset variables

# 0.0.9 (2023-06-17)

## Added
- `pk pkg`: add support for path-based targets
- `pk pkg source`: support sorting in ascending order via `--sort`
- `pk repo eapis`: add subcommand to show EAPI usage
- `pk repo leaf`: add subcommand to output leaf packages
- `pk repo metadata`: show progress bar during cache validation phase

## Changed
- Using stdin with relevant commands requires an initial arg of `-`.
- Log events are written to stderr instead of stdout.
- `pk pkg`: source ebuild from the current working directory by default
- `pk pkg source`
  - `-j/--jobs` defaults to # of physical CPUs
  - use human-time duration for `--bench` args

## Fixed
- Exit as expected when a SIGPIPE occurs (#112).

# 0.0.8 (2023-06-11)

## Added
- `pk pkg source`: support multiple `-b/--bound` args
- `pk repo metadata`: support multiple repo targets

## Changed
- Apply bounds to `-j/--jobs` args to be a positive integer that's less than or
  equal to a system's logical CPUs.
- Check for configured repos before trying to load one from a path.
- `pk pkg`: loop over targets performing a run for each
- `pk pkg source`: match against all configured ebuild repos by default
- `pk repo metadata: change `-r/--repo` option into a positional argument

# 0.0.7 (2023-06-04)

- initial release
