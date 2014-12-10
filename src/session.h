#pragma once
#include <libspotify/api.h>

extern uint8_t _binary_src_appkey_key_start;
extern uint8_t _binary_src_appkey_key_end;
extern size_t _binary_src_appkey_key_size;

sp_error session_init(sp_session **session);
