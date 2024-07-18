# mewbot
FAST bot for VRChat streamers on Twitch

It's written in pure rust.

The Discord module has yet to be implemented.

Until I feel it's ready for public use, you can download the source from the 2024-update branch and build it yourself with:

`cargo build`


When it runs for the first time it gives instructions and links for you get get the appropriate tokens from the various services it connects to.

All tokens and credentials are currently stored in plaintext in `mewbot.conf` which is located in the same folder as the built .exe

Will merge to main and post an executable package when it's more feature-complete.
