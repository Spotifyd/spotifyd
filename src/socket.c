/* communication with clients is handled here */

#include <stdio.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/queue.h>
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
		bytes_sent += send(sockfd, str + bytes_sent, strlen(str) - bytes_sent, 0);
	}
}

/*
 * sends a line containing information about track to 
 * the socket on sockfd.
 */
void sock_send_track_with_trackn(int sockfd, sp_track *track, int trackn)
{
	sp_artist *artist = sp_track_artist(track, 0);
	char str[API_MESSAGE_LEN];
	snprintf(str, API_MESSAGE_LEN, "%d: %s - %s\n", trackn, sp_track_name(track), sp_artist_name(artist));
	sock_send_str(sockfd, str);
}

void sock_send_track(int sockfd, sp_track *track)
{
	sp_artist *artist = sp_track_artist(track, 0);
	char str[API_MESSAGE_LEN];
	snprintf(str, API_MESSAGE_LEN, "%s - %s\n", sp_track_name(track), sp_artist_name(artist));
	sock_send_str(sockfd, str);
}

/*
 * creates a non blocking unix socket.
 */
int sock_create_un()
{
	int len, s;
	struct sockaddr_un local_un;

	s = socket(AF_UNIX, SOCK_STREAM, 0);
	local_un.sun_family = AF_UNIX;
	strcpy(local_un.sun_path, SOCKET_PATH);
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

	int flags = fcntl(s, F_GETFL, 0);
	fcntl(s, F_SETFL, flags | O_NONBLOCK);

	return s;
}

/*
 * Accepts connections to socket.
 */
void *sock_accept_connections(void *sock)
{
	int s = sock_create_un();

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
		if( (s2 = accept(s, (struct sockaddr *) &remote_un, &remote_un_s)) != -1)
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
	fprintf(stderr, "sock_connection_handler: malloc done.\n");
	if(!sock_readline(sockfd, string))
	{
		struct commandq_entry *entry = malloc(sizeof(struct commandq_entry));
		if(parse_input_line(entry, string, sockfd) != NULL)
		{
			pthread_mutex_lock(&commandq_lock);
			fprintf(stderr, "sock_connection_handler: took mutex.\n");
			commandq_insert(entry);
			fprintf(stderr, "sock_connection_handler: inserted entry.\n");
			pthread_mutex_unlock(&commandq_lock);
			fprintf(stderr, "sock_connection_handler: released mutex.\n");
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
