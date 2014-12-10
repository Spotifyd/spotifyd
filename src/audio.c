/*
 * Copyright (c) 2010 Spotify Ltd
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 *
 *
 * Audio helper functions.
 *
 * This file is part of the libspotify examples suite.
 */

#include "audio.h"
#include <stdlib.h>

audio_fifo_data_t* audio_get(audio_fifo_t *af)
{
    audio_fifo_data_t *afd;
    pthread_mutex_lock(&af->mutex);
  
    while (!(afd = TAILQ_FIRST(&af->q)))
	pthread_cond_wait(&af->cond, &af->mutex);
  
    TAILQ_REMOVE(&af->q, afd, link);
    af->qlen -= afd->nsamples;
  
    pthread_mutex_unlock(&af->mutex);
    return afd;
}

void audio_fifo_flush(audio_fifo_t *af)
{
    audio_fifo_data_t *afd;


    pthread_mutex_lock(&af->mutex);

    while((afd = TAILQ_FIRST(&af->q))) {
	TAILQ_REMOVE(&af->q, afd, link);
	free(afd);
    }

    af->qlen = 0;
    pthread_mutex_unlock(&af->mutex);
}
