package log

import (
	"encoding/json"
	"errors"
	"strings"
)

// LogLevelMapping is a mapping for LogLevel enum
var LogLevelMapping = map[string]LogLevel{
	ERROR.String():   ERROR,
	WARNING.String(): WARNING,
	INFO.String():    INFO,
	DEBUG.String():   DEBUG,
	SILENT.String():  SILENT,
}

const (
	DEBUG LogLevel = iota
	INFO
	WARNING
	ERROR
	SILENT
)

type LogLevel int

// UnmarshalYAML unserialize LogLevel with yaml
func (l *LogLevel) UnmarshalYAML(unmarshal func(any) error) error {
	var tp string
	unmarshal(&tp)
	level, exist := LogLevelMapping[strings.ToLower(tp)]
	if !exist {
		return errors.New("invalid mode")
	}
	*l = level
	return nil
}

// UnmarshalJSON unserialize LogLevel with json
func (l *LogLevel) UnmarshalJSON(data []byte) error {
	var tp string
	json.Unmarshal(data, &tp)
	level, exist := LogLevelMapping[strings.ToLower(tp)]
	if !exist {
		return errors.New("invalid mode")
	}
	*l = level
	return nil
}

// UnmarshalText unserialize LogLevel
func (l *LogLevel) UnmarshalText(data []byte) error {
	level, exist := LogLevelMapping[strings.ToLower(string(data))]
	if !exist {
		return errors.New("invalid mode")
	}
	*l = level
	return nil
}

// MarshalYAML serialize LogLevel with yaml
func (l LogLevel) MarshalYAML() (any, error) {
	return l.String(), nil
}

// MarshalJSON serialize LogLevel with json
func (l LogLevel) MarshalJSON() ([]byte, error) {
	return json.Marshal(l.String())
}

// MarshalText serialize LogLevel
func (l LogLevel) MarshalText() ([]byte, error) {
	return []byte(l.String()), nil
}

func (l LogLevel) String() string {
	switch l {
	case INFO:
		return "info"
	case WARNING:
		return "warning"
	case ERROR:
		return "error"
	case DEBUG:
		return "debug"
	case SILENT:
		return "silent"
	default:
		return "unknown"
	}
}
