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
#pragma once

#include <libspotify/api.h>

void command_prev(sp_session *session, const struct command * const command);
void command_next(sp_session *session, const struct command * const command);
void command_qprint(const struct command * const command);
void command_search(sp_session *session, const struct command * const command);
void command_qrand(sp_session *session, const struct command * const command);
void command_lists(sp_session *session, const struct command * const command);
void command_listq(sp_session *session, const struct command * const command);
void command_qadd(sp_session *session, const struct command * const command);
void command_play(sp_session *session, const struct command * const command);
void command_pause(sp_session *session, const struct command * const command);
void command_pl(const struct command * const command);
void command_saddpl(const struct command * const command);
void command_qaddpl(const struct command * const command);
void command_pladd(sp_session *session, const struct command * const command);
void command_plrm(const struct command * const command);
void command_plcreate(const struct command * const command);
void command_pldelete(const struct command * const command);
void command_cur_playing(const struct command * const command);
