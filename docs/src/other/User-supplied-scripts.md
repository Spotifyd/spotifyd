# User supplied Scripts

## Dunst Notifications (Using Spotify API)

This script will show a dunst notification when you play/change/stop Spotify (and when the music change). It is using spotify APIs to get music details.

### Dependencies

* curl (Request to APIs)
* xargs (Argument passing)
* cut
* jq (https://stedolan.github.io/jq) (for JSON parsing)

### How to use

* Create a file containing the script below:

    ```bash
    user_id=YOUR_USER_ID # generated on https://developer.spotify.com/dashboard/applications
    secret_id=YOUR_SECRET_ID

    myToken=$(curl -s -X 'POST' -u $user_id:$secret_id -d grant_type=client_credentials https://accounts.spotify.com/api/token | jq '.access_token' | cut -d\" -f2)
    RESULT=$?

    if [ "$PLAYER_EVENT" = "start" ];
    then
        if [ $RESULT -eq 0 ]; then
            curl -s -X 'GET' https://api.spotify.com/v1/tracks/$TRACK_ID -H 'Accept: application/json' -H 'Content-Type: application/json' -H "Authorization:\"Bearer $myToken\"" | jq '.name, .artists[].name, .album.name, .album.release_date, .track_number, .album.total_tracks' | xargs printf "\"Playing '%s' from '%s' (album: '%s' in %s (%s/%s))\"" | xargs notify-send --urgency=low --expire-time=3000 --icon=/usr/share/icons/gnome/32x32/actions/player_play.png --app-name=spotifyd spotifyd
        else
            echo "Cannot get token."
        fi
    elif [ "$PLAYER_EVENT" = "change" ];
    then
        if [ $RESULT -eq 0 ]; then
            curl -s -X 'GET' https://api.spotify.com/v1/tracks/$TRACK_ID -H 'Accept: application/json' -H 'Content-Type: application/json' -H "Authorization:\"Bearer $myToken\"" | jq '.name, .artists[].name, .album.name, .album.release_date, .track_number, .album.total_tracks' | xargs printf "\"Music changed to '%s' from '%s' (album: '%s' in %s (%s/%s))\"" | xargs notify-send --urgency=low --expire-time=3000 --icon=/usr/share/icons/gnome/32x32/actions/player_fwd.png --app-name=spotifyd spotifyd
        else
            echo "Cannot get token."
        fi
    elif [ "$PLAYER_EVENT" = "stop" ];
    then
        if [ $RESULT -eq 0 ]; then
            curl -s -X 'GET' https://api.spotify.com/v1/tracks/$TRACK_ID -H 'Accept: application/json' -H 'Content-Type: application/json' -H "Authorization:\"Bearer $myToken\"" | jq '.name, .artists[].name, .album.name, .album.release_date, .track_number, .album.total_tracks' | xargs printf "Stoping music (Last song: '%s' from '%s' (album: '%s' in %s (%s/%s)))\"" | xargs notify-send --urgency=low --expire-time=3000 --icon=/usr/share/icons/gnome/32x32/actions/player_stop.png --app-name=spotifyd spotifyd
        else
            echo "Cannot get token."
        fi
    else
        echo "Unknown event."
    fi
    ```

* Make this script executable (```chmod +x ntification_script.sh```)
* Add the line ```onevent = "bash /home/YOU_USER/bin/spotifyNotifications.sh"``` to your ```spotifyd.conf```

## Dunst Notifications (Using Playerctl metadata)

This script is a modification of the script supplied above, however instead of calling the Spotify API for track information, the metadata of the current track is used instead, leading to a more performant script.

### Dependencies

* [Playerctl](https://github.com/altdesktop/playerctl)

### How to Use:

* Download this [gist](https://gist.github.com/ohhskar/efe71e82337ed54b9aa704d3df28d2ae)
* Make the script executable (```chmod +x notifications.sh```)
* Add the line ```onevent = "/path/to/file/spotifyNotifications.sh"``` to your ```spotifyd.conf```