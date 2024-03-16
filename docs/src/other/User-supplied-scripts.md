# User supplied Scripts

The config value `on_song_change_hook` or `onevent` can be used to hook into player events. The command is executed with different environment variables depending on the `$PLAYER_EVENT`.

## $PLAYER_EVENT values

### change
Send when the track changes.
* `$OLD_TRACK_ID`
* `$TRACK_ID`

### start
Sent when a (client connects / session is started).
* `$TRACK_ID`
* `$PLAY_REQUEST_ID`
* `$POSITION_MS`

### stop
Sent when a (client disconnects / session is stopped).
* `$TRACK_ID`
* `$PLAY_REQUEST_ID`

### load
Sent when a track is loaded.
* `$TRACK_ID`
* `$PLAY_REQUEST_ID`
* `$POSITION_MS`

### play
Sent when a track starts to play.
* `$TRACK_ID`
* `$PLAY_REQUEST_ID`
* `$POSITION_MS`
* `$DURATION_MS`

### pause 
Sent when a track is paused.
* `$TRACK_ID`
* `$PLAY_REQUEST_ID`
* `$POSITION_MS`
* `$DURATION_MS`

### preload
Sent a few seconds before the end of the track.
* `$TRACK_ID`
* `$PLAY_REQUEST_ID`

### endoftrack
Sent at the end of the track.
* `$TRACK_ID`
* `$PLAY_REQUEST_ID`

### volumeset
Sent when volume is set.
* `$VOLUME`

### unavailable
Sent when a track is unavailable.
* `$TRACK_ID`
* `$PLAY_REQUEST_ID`

### preloading
Sent after `$PLAYER_EVENT=preload`.
* `$TRACK_ID`

## How to use
* Create a script ```/path/to/file/spotifyNotifications.sh```
* Make the script executable (```chmod +x /path/to/file/spotifyNotifications.sh```)
* Add the line ```onevent = "/path/to/file/spotifyNotifications.sh"``` to your ```spotifyd.conf```


## Notifications with album cover (using Spotify API)

This script will show a notification when a track is played. It is using spotify [Web API](https://developer.spotify.com/documentation/web-api) to get music details.


### Dependencies

* curl 
* wget
* jq 
* notify-send


### Script

```bash
#!/bin/bash
# spotify api credentials; generated on https://developer.spotify.com/dashboard/applications
user_id=YOUR_USER_ID
secret_id=YOUR_SECRET_ID

# get access token
token=$(curl -s -X 'POST' -u "$user_id:$secret_id" -d grant_type=client_credentials https://accounts.spotify.com/api/token | jq -r '.access_token')

if [ "$token" = "" ]
then
    notify-send --urgency=critical "$(basename "$0"): error fetching spotify api token"
    exit 1
fi

cover_file=/tmp/spotifyd-cover.jpg
track_json=/tmp/spotifyd-track.json

if [ "$PLAYER_EVENT" = "change" ] || [ "$PLAYER_EVENT" = "start" ];
then
    # load json of current track
    curl -s -X 'GET' https://api.spotify.com/v1/tracks/"$TRACK_ID" -H 'Accept: application/json' -H 'Content-Type: application/json' -H "Authorization:\"Bearer $token\"" > "$track_json"
    # load image of cover
    cover_url=$(jq -r '.album.images[0].url' "$track_json")
    wget -O "$cover_file" "$cover_url"
elif [ "$PLAYER_EVENT" = "play" ];
then
    # parse track info
    title=$(jq -r '.name' "$track_json")
    artists=$(jq -r '.artists | map(.name) | join(", ")' "$track_json")
    album=$(jq -r '.album.name' "$track_json")
    # send notification
    notify-send --urgency=low --expire-time=5000 --icon="$cover_file" --app-name=spotifyd "$title" "$artist\n$album"
fi
```


## Notifications (using playerctl metadata)

This script is an updated version of this [gist](https://gist.github.com/ohhskar/efe71e82337ed54b9aa704d3df28d2ae). It uses playerctl metadata instead of the Spotify API.


### Dependencies

* [Playerctl](https://github.com/altdesktop/playerctl)
* notify-send

### Script

```bash
#!/bin/bash

if [ "$PLAYER_EVENT" = "play" ] || [ "$PLAYER_EVENT" = "change" ];
then
	trackName=$(playerctl -p spotifyd,%any metadata title)
	artistAndAlbumName=$(playerctl -p spotifyd,%any metadata --format "{{ artist }} ({{ album }})")

	notify-send -u low "$trackName" "$artistAndAlbumName "
fi
```
