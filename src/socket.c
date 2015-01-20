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
/* communication with clients is handled here */

#include <stdio.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/queue.h>
#include <netinet/in.h>
#include <netdb.h>
#include <sys/un.h>
#include <fcntl.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

#include "spotifyd.h"
#include "socket.h"
#include "parser.h"
#include "helpers.h"

pthread_t thread;
int s_ip, s_un;

/*
 * reads from sockfd into buf until either
 * a newline character is found or API_MESSAGE_LEN
 * bytes have been read.
 */
int sock_readline(int sockfd, char *buf)
{
	unsigned bytes_read = 0;
	int tmp;
	do
	{
		if( (tmp = recv(sockfd, (buf + bytes_read), API_MESSAGE_LEN - 1 - bytes_read, 0)) == -1)
		{
			return 1;
		}
		else if(tmp == 0)
		{
			return 1;
		}
		else
		{
			bytes_read += tmp;
		}
	}while(buf[bytes_read - 1] != '\n');

	buf[bytes_read-1] = '\0';
	return 0;
}

/*
 * send the NULL-terminated string str
 * to the socket on sockfd.
 */
void sock_send_str(int sockfd, const char * const str)
{
	int bytes_sent = 0;
	while(bytes_sent < strlen(str))
	{
		bytes_sent += send(sockfd, str + bytes_sent, strlen(str) - bytes_sent, MSG_NOSIGNAL);
	}
}

/*
 * sends a line containing information about track to 
 * the socket on sockfd.
 */
void sock_send_track_with_trackn(int sockfd, sp_track *track, int trackn)
{
	if(track != NULL && sp_track_error(track) == SP_ERROR_OK)
	{
		sp_artist *artist = sp_track_artist(track, 0);
		char str[API_MESSAGE_LEN];
		sp_link *l = sp_link_create_from_track(track, 0);
		snprintf(str, API_MESSAGE_LEN, "%d | %s | %s | ", trackn, sp_track_name(track), sp_artist_name(artist));
		sp_link_as_string(l, str + strlen(str), API_MESSAGE_LEN - strlen(str));
		sp_link_release(l);
		sock_send_str(sockfd, str);
	}
	else if(track != NULL && sp_track_error(track) == SP_ERROR_IS_LOADING)
	{
		sock_send_str(sockfd, "Track is loading, try again.");
	}
}

void sock_send_track(int sockfd, sp_track *track)
{
	if(track != NULL && sp_track_error(track) == SP_ERROR_OK)
	{
		sp_artist *artist = sp_track_artist(track, 0);
		char str[API_MESSAGE_LEN];
		sp_link *l = sp_link_create_from_track(track, 0);
		snprintf(str, API_MESSAGE_LEN, "%s | %s | ", sp_track_name(track), sp_artist_name(artist));
		sp_link_as_string(l, str + strlen(str), API_MESSAGE_LEN - strlen(str));
		sp_link_release(l);
		sock_send_str(sockfd, str);
	}
	else if(track != NULL && sp_track_error(track) == SP_ERROR_IS_LOADING)
	{
		sock_send_str(sockfd, "Track is loading, try again.");
	}
}

/*
 * creates an unix socket.
 */
int sock_create_un()
{
	int len, s;
	struct sockaddr_un local_un;

	s = socket(AF_UNIX, SOCK_STREAM, 0);
	local_un.sun_family = AF_UNIX;
	char *path = get_socket_path();
	strcpy(local_un.sun_path, path);
	free(path);
	unlink(local_un.sun_path);
	len = strlen(local_un.sun_path) + sizeof(local_un.sun_family);

	if(bind(s, (struct sockaddr *) &local_un, len) == -1)
	{
		perror("bind");
		exit(1);
	}

	if(listen(s, 10) == -1)
	{
		perror("listen");
		exit(1);
	}

	return s;
}

/*
 * creates an IP socket.
 */
int sock_create_ip()
{
	struct sockaddr_storage;
	struct addrinfo hints, *res;
	int sockfd;

	memset(&hints, 0, sizeof hints);
	hints.ai_family = AF_UNSPEC;
	hints.ai_socktype = SOCK_STREAM;
	hints.ai_flags = AI_PASSIVE;

	char *port = get_port();
	getaddrinfo(NULL, port, &hints, &res);
	free(port);

	sockfd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
	if(bind(sockfd, res->ai_addr, res->ai_addrlen) == -1)
	{
		perror("bind");
		exit(1);
	}

	if(listen(sockfd, 10) == -1)
	{
		perror("listen");
		exit(1);
	}

	return sockfd;
}

/*
 * Accepts connections to socket.
 */
void *sock_accept_connections_un(void *not_used)
{
	s_un = sock_create_un();

	/* 
	 * A bit of a hack. Makes s2 an unsigned integer
	 * with the same length as a pointer. That means we
	 * can cast it to a void pointer and avoid heap-allocating
	 * an integer to send to the new thread.
	 */
	uintptr_t s2;
	struct sockaddr_un remote_un;
	unsigned remote_un_s = sizeof(remote_un);

	for(;;)
	{
		if( (s2 = accept(s_un, (struct sockaddr *) &remote_un, &remote_un_s)) != -1)
		{
			/* 
			 * we got someone connected. send them to the
			 * connection handler.
			 */
			pthread_create(&thread, NULL, sock_connection_handler, (void*) s2);
		}
	}

	return NULL;
}

void *sock_accept_connections_ip(void *not_used)
{
	int s_ip = sock_create_ip();

	/* 
	 * A bit of a hack. Makes s2 an unsigned integer
	 * with the same length as a pointer. That means we
	 * can cast it to a void pointer and avoid heap-allocating
	 * an integer to send to the new thread.
	 */
	uintptr_t s2;
	struct sockaddr_storage their_addr;
	socklen_t addr_size = sizeof their_addr;
	for(;;)
	{
		if( (s2 = accept(s_ip, (struct sockaddr *) &their_addr, &addr_size)) != -1)
		{
			/* 
			 * we got someone connected. send them to the
			 * connection handler.
			 */
			pthread_create(&thread, NULL, sock_connection_handler, (void*) s2);
		}
	}

	return NULL;
}

	
/*
 * Reads commands from the socket fd pointed to by sock.
 * Will add valid commands to the command queue. If
 * the users gives an invalid command, the socket is
 * closed. Otherwise, the socket will close when the
 * command has finished.
 */
void *sock_connection_handler(void *sock)
{
	int sockfd = (uintptr_t)sock;
	
	char *string = malloc(sizeof(char) * API_MESSAGE_LEN);
	if(string != NULL && !sock_readline(sockfd, string))
	{
		struct commandq_entry *entry = malloc(sizeof(struct commandq_entry));
		if(entry != NULL && parse_input_line(entry, string, sockfd) != NULL)
		{
			pthread_mutex_lock(&commandq_lock);
			commandq_insert(entry);
			pthread_mutex_unlock(&commandq_lock);
			notify_main_thread();
		}
		else
		{
			free(entry);
			sock_send_str(sockfd, "not a valid command.\n");
			close(sockfd);
		}
	}

	free(string);

	return NULL;
}

void sock_close()
{
	close(s_ip);
	close(s_un);
}
