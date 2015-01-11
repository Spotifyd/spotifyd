/*
 * spotifyd - A daemon playing music from spotify, in the spirit of MPD.
 * Copyright (C) 2015 Simon Persson
 * 
 * Spotifyd program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Spotifyd program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
#include <libspotify/api.h>
#include <pthread.h>

#include "search.h"
#include "config.h"

sp_track *search_result[NUM_SEARCH_RESULTS];

/*
 * clear search list. Caller must take search_result_lock before calling.
 */
void search_clear()
{
	int i;
	for(i=0; i<NUM_SEARCH_RESULTS; ++i)
	{
		if(search_result[i] != NULL)
		{
			sp_track_release(search_result[i]);
			search_result[i] = NULL;
		}
	}
}

/*
 * add track to search list, caller must tack search_result_lock before calling.
 */
bool search_add_track(sp_track *track)
{
	sp_track_add_ref(track);

	/*
	 * store track at first free space.
	 */
	int i = 0;
	while(i<NUM_SEARCH_RESULTS && search_result[i] != NULL) ++i;
	if(i == NUM_SEARCH_RESULTS)
	{
		return 0;
	}
	else
	{
		search_result[i] = track;
		return 1;
	}
}

sp_track *search_get(unsigned i)
{
	if(i<NUM_SEARCH_RESULTS)
		return search_result[i];
	else
		return NULL;
}
