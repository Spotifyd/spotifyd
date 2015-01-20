#!/usr/bin/env python

import os
import subprocess
import pyen
from time import sleep
from threading import Thread

songs = {}

def add_song(song):
    global songs
    if len(song['tracks']) > 0:
        subprocess.call("sc link " + song['tracks'][0]['foreign_id'] + ">/dev/null", shell=True)
        songs[song['tracks'][0]['foreign_id']] = song['id']
        subprocess.call("sc qadd 0 >/dev/null", shell=True)

def play_first():
    subprocess.call("sc play 0 > /dev/null", shell=True)

def play_next():
    subprocess.call("sc next > /dev/null", shell=True)

def clear_queue():
    subprocess.call("sc qclear >/dev/null", shell=True)

def get_current_song():
    p = subprocess.Popen("sc cur_playing", shell=True, stdout=subprocess.PIPE)
    out, err = p.communicate()
    fields = out.split('|')
    if len(fields) == 3:
        return fields[2].strip()
    else:
        return None

#configure spotifyd to print the currently playing song first.
def set_print_mode():
    p = subprocess.Popen("sc qprint", shell=True, stdout=subprocess.PIPE)
    out, err = p.communicate()
    if "Will print the first song in queue first" in out:
        set_print_mode()

def songs_in_queue():
    p = subprocess.Popen("sc qlist", shell=True, stdout=subprocess.PIPE)
    out, err = p.communicate()
    return len(out.split('\n'))

def get_seed(type):
    return raw_input("Enter "+type+" to use as seed for recomending more music: ")

def playlist_get_next(session_id):
    return en.get('playlist/dynamic/next', session_id=session_id)['songs'][0]

def playlist_iter(session_id):
    while True:
        yield playlist_get_next(session_id) 

def find_track_id(songs, artist, track):
    return None

#make sure there's always two songs in queue
#by polling for change of currently playing
#song every second.
def update_playlist(session_id):
    sleep(1)
    for song in playlist_iter(session_id):
        while songs_in_queue() >= 3:
            sleep(1)
        add_song(song)

en = pyen.Pyen("TANPJV5OAIXMBC5TR")
if raw_input("Genre or artist radio? ").lower() == "artist":
    session_id = en.get('playlist/dynamic/create', artist=get_seed("artist"), bucket=["id:spotify", "tracks"], type='artist-radio')['session_id']
else:
    session_id = en.get('playlist/dynamic/create', genre=get_seed("genre"), bucket=["id:spotify", "tracks"], type='genre-radio')['session_id']

clear_queue()
#begin with 1 songs in queue.
add_song(playlist_get_next(session_id))
set_print_mode()
play_first()

thread = Thread(target = update_playlist, args = (session_id, ))
thread.daemon = True
thread.start()

while True:
    feedback = raw_input("Feedback: ")
        
    spotify_id = get_current_song()
    if spotify_id != None:
        echonest_id = songs[spotify_id]
        #if user types skip, send feedback to echonest
        #and skip song
        if feedback == "skip":
            if echonest_id != None:
                en.get('playlist/dynamic/feedback', session_id=session_id, skip_song=echonest_id)
            play_next()
        
        #if user gives a number as feedback, send it to echo nest
        if feedback.isdigit() and echonest_id != None:
                en.get('playlist/dynamic/feedback', session_id=session_id, rate_song=str(echonest_id)+'^'+feedback)
