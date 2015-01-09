#pragma once

#include <libspotify/api.h>

void command_search(sp_session *session, const struct command * const command);
void command_qrand(sp_session *session, const struct command * const command);
void command_lists(sp_session *session, const struct command * const command);
void command_listq(sp_session *session, const struct command * const command);
void command_qadd(sp_session *session, const struct command * const command);
void command_play(sp_session *session, const struct command * const command);
