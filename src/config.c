#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <ctype.h>

#include "spotifyd.h"
#include "config.h"

char *socket_path = NULL;
char *username = NULL;
char *password = NULL;
char *port = NULL;

/*
 * We only want to get these values
 * once so they are set to NULL. The
 * caller frees them.
 */

char *get_socket_path()
{
	char *tmp = socket_path;
	socket_path = NULL;
	return tmp;
}

char *get_port()
{
	char *tmp = port;
	port = NULL;
	return tmp;
}

char *get_username()
{
	char *tmp = username;
	username = NULL;
	return tmp;
}

char *get_password()
{
	char *tmp = password;
	password = NULL;
	return tmp;
}

bool have_port()
{
	return port != NULL;
}

bool have_socket_path()
{
	return socket_path != NULL;
}

char *trim_whitespace(char *str)
{
	/*
	 * remove leading whitespace.
	 */
	while(isspace(*str)) ++str;

	char *end = str + strlen(str) - 1;
	while(isspace(*end)) --end;
	*(end + 1) = '\0';

	return str;
}

bool read_config()
{
	char *config_file = malloc(sizeof(char) * (strlen(getenv("HOME")) + strlen("/.spotifyd.rc") + 1));
	strcat(config_file, getenv("HOME"));
	strcat(config_file, "/.spotifyd.rc");
	FILE *fp = fopen(config_file, "r");
	free(config_file);
	char *line = NULL;
	size_t n = 0;

	while(fp != NULL && -1 != getline(&line, &n, fp))
	{
		if(!strncasecmp(line, "username", strlen("username")))
		{
			char *tmp = trim_whitespace(line + strlen("username"));
			username = malloc(sizeof(char) * (strlen(tmp) + 1));
			strcpy(username, tmp);
		}
		else if(!strncasecmp(line, "password", strlen("password")))
		{
			char *tmp = trim_whitespace(line + strlen("password"));
			password = malloc(sizeof(char) * (strlen(tmp) + 1));
			strcpy(password, tmp);
		}
		else if(!strncasecmp(line, "unix-socket", strlen("unix-socket")))
		{
			char *tmp = trim_whitespace(line + strlen("unix-socket"));
			socket_path = malloc(sizeof(char) * (strlen(tmp) + 1));
			strcpy(socket_path, tmp);
		}
		else if(!strncasecmp(line, "port", strlen("port")))
		{
			char *tmp = trim_whitespace(line + strlen("port"));
			port = malloc(sizeof(char) * (strlen(tmp) + 1));
			strcpy(port, tmp);
		}

		free(line);
		n = 0;
	}
	
	if(username == NULL)
	{
		fprintf(stderr, "Couldn't read username!\n");
		exit(-1);
	}
	if(password == NULL)
	{
		fprintf(stderr, "Couldn't read password!\n");
		exit(-1);
	}

	return 1;
}
