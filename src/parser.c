#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#include "commandq.h"

struct commandq_entry *parse_input_line(struct commandq_entry *entry, const char * const line, int sockfd)
{
	struct commandq_entry *ret_val = NULL;
	struct command *command = malloc(sizeof(struct command));
	command->handled = 0;
	command->done = 0;
	command->sockfd = sockfd;
	entry->val = command;
	if(!strncasecmp(line, "search ", strlen("search ")))
	{
		command->type = SEARCH;
		command->search_string = malloc(sizeof(char) * strlen(line + strlen("search ")) + 1);
		strcpy(command->search_string, line + strlen("search "));

		ret_val = entry;
	}
	else if(!strncasecmp(line, "qlist", strlen("qlist")))
	{
		command->type = QLIST;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "slist", strlen("slist")))
	{
		command->type = SLIST;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "qrand", strlen("qrand")))
	{
		command->type = QRAND;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "help", strlen("help")))
	{
		command->type = HELP;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "qclear", strlen("qclear")))
	{
		command->type = QCLEAR;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "qadd ", strlen("qadd ")))
	{
		command->type = QADD;
		char *tmp;
		command->track = strtol(line + strlen("qadd "), &tmp, 10);
		if(*tmp == '\0')
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "qrm ", strlen("qrm ")))
	{
		command->type = QRM;
		char *tmp;
		command->track = strtol(line + strlen("qrm "), &tmp, 10);
		if(*tmp == '\0')
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "play ", strlen("play ")))
	{
		command->type = PLAY;
		char *tmp;
		command->track = strtol(line + strlen("qplay "), &tmp, 10);
		if(*tmp == '\0')
		{
			ret_val = entry;
		}
	}

	if(ret_val == NULL)
	{
		free(command);
	}
	return ret_val;
}
