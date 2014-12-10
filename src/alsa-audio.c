/*
 * Copyright (c) 2006-2009 Spotify Ltd
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
 * ALSA audio output driver.
 *
 * This file is part of the libspotify examples suite.
 */

#include <asoundlib.h>
#include <errno.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/time.h>

#include "audio.h"


static snd_pcm_t *alsa_open(char *dev, int rate, int channels)
{
	snd_pcm_hw_params_t *hwp;
	snd_pcm_sw_params_t *swp;
	snd_pcm_t *h;
	int r;
	int dir;
	snd_pcm_uframes_t period_size_min;
	snd_pcm_uframes_t period_size_max;
	snd_pcm_uframes_t buffer_size_min;
	snd_pcm_uframes_t buffer_size_max;
	snd_pcm_uframes_t period_size;
	snd_pcm_uframes_t buffer_size;

	if ((r = snd_pcm_open(&h, dev, SND_PCM_STREAM_PLAYBACK, 0) < 0))
		return NULL;

	hwp = alloca(snd_pcm_hw_params_sizeof());
	memset(hwp, 0, snd_pcm_hw_params_sizeof());
	snd_pcm_hw_params_any(h, hwp);

	snd_pcm_hw_params_set_access(h, hwp, SND_PCM_ACCESS_RW_INTERLEAVED);
	snd_pcm_hw_params_set_format(h, hwp, SND_PCM_FORMAT_S16_LE);
	snd_pcm_hw_params_set_rate(h, hwp, rate, 0);
	snd_pcm_hw_params_set_channels(h, hwp, channels);

	/* Configurue period */

	dir = 0;
	snd_pcm_hw_params_get_period_size_min(hwp, &period_size_min, &dir);
	dir = 0;
	snd_pcm_hw_params_get_period_size_max(hwp, &period_size_max, &dir);

	period_size = 1024;

	dir = 0;
	r = snd_pcm_hw_params_set_period_size_near(h, hwp, &period_size, &dir);

	if (r < 0) {
		fprintf(stderr, "audio: Unable to set period size %lu (%s)\n",
		        period_size, snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	dir = 0;
	r = snd_pcm_hw_params_get_period_size(hwp, &period_size, &dir);

	if (r < 0) {
		fprintf(stderr, "audio: Unable to get period size (%s)\n",
		        snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	/* Configurue buffer size */

	snd_pcm_hw_params_get_buffer_size_min(hwp, &buffer_size_min);
	snd_pcm_hw_params_get_buffer_size_max(hwp, &buffer_size_max);
	buffer_size = period_size * 4;

	dir = 0;
	r = snd_pcm_hw_params_set_buffer_size_near(h, hwp, &buffer_size);

	if (r < 0) {
		fprintf(stderr, "audio: Unable to set buffer size %lu (%s)\n",
		        buffer_size, snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	r = snd_pcm_hw_params_get_buffer_size(hwp, &buffer_size);

	if (r < 0) {
		fprintf(stderr, "audio: Unable to get buffer size (%s)\n",
		        snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	/* write the hw params */
	r = snd_pcm_hw_params(h, hwp);

	if (r < 0) {
		fprintf(stderr, "audio: Unable to configure hardware parameters (%s)\n",
		        snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	/*
	 * Software parameters
	 */

	swp = alloca(snd_pcm_sw_params_sizeof());
	memset(hwp, 0, snd_pcm_sw_params_sizeof());
	snd_pcm_sw_params_current(h, swp);

	r = snd_pcm_sw_params_set_avail_min(h, swp, period_size);

	if (r < 0) {
		fprintf(stderr, "audio: Unable to configure wakeup threshold (%s)\n",
		        snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	snd_pcm_sw_params_set_start_threshold(h, swp, 0);

	if (r < 0) {
		fprintf(stderr, "audio: Unable to configure start threshold (%s)\n",
		        snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	r = snd_pcm_sw_params(h, swp);

	if (r < 0) {
		fprintf(stderr, "audio: Cannot set soft parameters (%s)\n",
		snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	r = snd_pcm_prepare(h);
	if (r < 0) {
		fprintf(stderr, "audio: Cannot prepare audio for playback (%s)\n",
		snd_strerror(r));
		snd_pcm_close(h);
		return NULL;
	}

	return h;
}

static void* alsa_audio_start(void *aux)
{
	audio_fifo_t *af = aux;
	snd_pcm_t *h = NULL;
	int c;
	int cur_channels = 0;
	int cur_rate = 0;

	audio_fifo_data_t *afd;

	for (;;) {
		afd = audio_get(af);

		if (!h || cur_rate != afd->rate || cur_channels != afd->channels) {
			if (h) snd_pcm_close(h);

			cur_rate = afd->rate;
			cur_channels = afd->channels;

			h = alsa_open("default", cur_rate, cur_channels);

			if (!h) {
				fprintf(stderr, "Unable to open ALSA device (%d channels, %d Hz), dying\n",
				        cur_channels, cur_rate);
				exit(1);
			}
		}

		c = snd_pcm_wait(h, 1000);

		if (c >= 0)
			c = snd_pcm_avail_update(h);

		if (c == -EPIPE)
			snd_pcm_prepare(h);

		snd_pcm_writei(h, afd->samples, afd->nsamples);
		free(afd);
	}
}

void audio_init(audio_fifo_t *af)
{
	pthread_t tid;

	TAILQ_INIT(&af->q);
	af->qlen = 0;

	pthread_mutex_init(&af->mutex, NULL);
	pthread_cond_init(&af->cond, NULL);

	pthread_create(&tid, NULL, alsa_audio_start, af);
}

