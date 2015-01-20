# Scripts
In this directory I will collect all the cool things that people
do with spotifyd. Feel free to send a pull request if you have done
something cool that is missing.

## sc
A wrapper around `socat` or `nc` that connects to spotifyd. Both versions share the exact same
functionality but differs in how they connect to spotifyd, by unix socket or by network socket.

Put any of these in your `.profile`:
```
sc () {
	echo $@ | socat - UNIX-CONNECT:/tmp/spotifyd 2>/dev/null
}
```
or
```
sc () {
	echo $@ | nc 192.168.0.119 13337;
}
```
followed by
```
export -f sc
```
and change the location of the unix socket/the IP and port of the server.

This little shell function is required by every other script so make sure you have it
installed even though you don't plan on using it directly.

## spotifyd-dmenu
Searches the queue with dmenu and plays the selected song. Provided by
[Sævar Berg](http://www.github.com/saevarb).

## spotifyd-nowplaying
Parses `sc cur_playing` and allows for customization of the output as well
as truncation and such. Ideal for trays/bars. See file for more details.
Provided by [Sævar Berg](http://www.github.com/saevarb).

## scli
Wrapper script for `sc`, keeps reading input from user so that
you don't have to retype the `sc` command. Made by [MacGuyverism](http://www.reddit.com/user/MacGuyverism)
on reddit and modified slightly by [SimonPersson](https://github.com/SimonPersson).
