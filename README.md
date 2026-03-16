# Gnome Style Workspace Indicator for Cosmic
At the moment it is just a gnome style workspace indicator. I did this to learn how to customize the indicator. I would love to set it up to be customizable through a config file or a settings page but I don't know if I will get to that. This is a fork of the cosmic numbered workspace indicator. I have never used rust before this and I am just figuring it out so the code is probably not optimal.

**Feel Free to contribute and improve this. I don't know how much time I will have to continue working on it.**

## install

To install run\
`Just`\
then\
`sudo just install_gnome_indicator`

For somen reason you will then have to copy the binary into the bin folder, I don't know why installing does not do that.\
`sudo cp target/release/gnome-style-workspace-indicator /usr/local/bin`

## known issues
- No animation, this I think is an iced/libcosmic limitation at the moment
- The highlight on hover is not based on the color set from the theme instead it is just a color that is simular at the moment
- The highlight does not correctly touch the top and bottom of the panel/dock like it does for all other applets
- Highlight does not cause cursor to turn into pointer unless over the actual indicator dots/dash
- It is not an exact copy just a loose approximation based on images and videos, I have never used Gnome outside of short tests
