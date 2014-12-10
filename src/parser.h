#pragma once
#include "commandq.h"

struct commandq_entry *parse_input_line(struct commandq_entry *entry, const char * const line, int sockfd);
