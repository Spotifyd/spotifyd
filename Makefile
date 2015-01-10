CC      := gcc
PKGS	:= libspotify alsa
CFLAGS  := `pkg-config --cflags $(PKGS)` -Wall -g
LIBS    := `pkg-config --libs $(PKGS)` -lpthread -pthread

TARGET	:= spotifyd
SOURCES := $(shell find src/ -type f -name *.c)
OBJECTS := $(patsubst src/%,build/%,$(SOURCES:.c=.o))
OBJECTS += build/appkey.o
DEPS	:= $(OBJECTS:.o=.deps)

$(TARGET): $(OBJECTS)
	@echo "  Linking '$(TARGET)'..."; $(CC) $^ -o $(TARGET) $(LIBS)

build/%.o: src/%.c
	@mkdir -p build/
	@echo "  CC $<"; $(CC) $(CFLAGS) -MD -MF $(@:.o=.deps) -c -o $@ $<

build/appkey.o: src/appkey.key
	ld -r -b binary -o build/appkey.o src/appkey.key

clean:
	@echo "  Cleaning..."; $(RM) -r build/ $(TARGET)

install: $(TARGET)
	@cp $(TARGET) ${DESTDIR}${PREFIX}/bin/

-include $(DEPS)

.PHONY: clean
