#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include <unistd.h>

#include "spotifyd.h"
#include "config.h"
#include "helpers.h"

char *socket_path = NULL;
char *username = NULL;
char *password = NULL;
char *port = NULL;
FILE *logfile = NULL;

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

/*
 * The following methods can be called multiple times.
 */

FILE *get_logfile()
{
	return logfile;
}


bool have_port()
{
	return port != NULL;
}

bool have_socket_path()
{
	return socket_path != NULL;
}

char *trim_whitespace_front(char *str)
{
	/*
	 * remove leading whitespace.
	 */
	while(isspace(*str)) ++str;
	return str;
}

char *trim_whitespace_back(char *str)
{
	char *end = str + strlen(str) - 1;
	while(isspace(*end)) --end;
	*(end + 1) = '\0';

	return str;
}

char *trim_whitespace(char *str)
{
	return trim_whitespace_back(trim_whitespace_front(str));
}

bool read_config()
{
	char *config_file = malloc(sizeof(char) * (strlen(getenv("HOME")) + strlen("/.spotifyd.rc") + 1));
	if(config_file == NULL)
	{
		LOG_PRINT("Can't allocate memory. Quitting.\n");
		exit(1);
	}
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
			if(username == NULL)
			{
				LOG_PRINT("Can't allocate memory. Quitting.\n");
				exit(1);
			}
			strcpy(username, tmp);
		}
		else if(!strncasecmp(line, "password", strlen("password")))
		{
			char *tmp = trim_whitespace(line + strlen("password"));
			password = malloc(sizeof(char) * (strlen(tmp) + 1));
			if(password == NULL)
			{
				LOG_PRINT("Can't allocate memory. Quitting.\n");
				exit(1);
			}
			strcpy(password, tmp);
		}
		else if(!strncasecmp(line, "unix-socket", strlen("unix-socket")))
		{
			char *tmp = trim_whitespace(line + strlen("unix-socket"));
			socket_path = malloc(sizeof(char) * (strlen(tmp) + 1));
			if(socket_path == NULL)
			{
				LOG_PRINT("Can't allocate memory. Quitting.\n");
				exit(1);
			}
			strcpy(socket_path, tmp);
		}
		else if(!strncasecmp(line, "port", strlen("port")))
		{
			char *tmp = trim_whitespace(line + strlen("port"));
			port = malloc(sizeof(char) * (strlen(tmp) + 1));
			if(port == NULL)
			{
				LOG_PRINT("Can't allocate memory. Quitting.\n");
				exit(1);
			}
			strcpy(port, tmp);
		}
		else if(!strncasecmp(line, "log", strlen("log")))
		{
			char *tmp = trim_whitespace(line + strlen("log"));
			logfile = fopen(tmp, "w");
		}
		free(line);
		n = 0;
	}
	if(username == NULL && password != NULL)
	{
		LOG_PRINT("Couldn't read username.\n");
		exit(-1);
	}
	else if(username == NULL && password == NULL)
	{
		n = 0;
		LOG_PRINT("Username: ");
		if(getline(&username, &n, stdin) == -1)
		{
			LOG_PRINT("Couldn't read line.\n");
			exit(-1);
		}
		else
		{
			username = trim_whitespace_back(username);
		}
		if((password = getpass("Password: ")) == NULL)
		{
			LOG_PRINT("Couldn't password.\n");
			exit(-1);
		}
	}
	else if(username != NULL && password == NULL)
	{
		if((password = getpass("Password: ")) == NULL)
		{
			LOG_PRINT("Couldn't password.\n");
			exit(-1);
		}
	}
	return 1;
}
