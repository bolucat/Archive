package lib

import (
	"encoding/json"
	"errors"
	"os"
	"strings"

	"github.com/tailscale/hujson"
)

type Instance struct {
	config *config
	input  []InputConverter
	output []OutputConverter
}

func NewInstance() (*Instance, error) {
	return &Instance{
		config: new(config),
		input:  make([]InputConverter, 0),
		output: make([]OutputConverter, 0),
	}, nil
}

func (i *Instance) Init(configFile string) error {
	var content []byte
	var err error
	configFile = strings.TrimSpace(configFile)
	if strings.HasPrefix(strings.ToLower(configFile), "http://") || strings.HasPrefix(strings.ToLower(configFile), "https://") {
		content, err = GetRemoteURLContent(configFile)
	} else {
		content, err = os.ReadFile(configFile)
	}
	if err != nil {
		return err
	}

	// Support JSON with comments and trailing commas
	content, _ = hujson.Standardize(content)

	if err := json.Unmarshal(content, &i.config); err != nil {
		return err
	}

	for _, input := range i.config.Input {
		i.input = append(i.input, input.converter)
	}

	for _, output := range i.config.Output {
		i.output = append(i.output, output.converter)
	}

	return nil
}

func (i *Instance) InitFromBytes(content []byte) error {
	// Support JSON with comments and trailing commas
	content, _ = hujson.Standardize(content)

	if err := json.Unmarshal(content, &i.config); err != nil {
		return err
	}

	for _, input := range i.config.Input {
		i.input = append(i.input, input.converter)
	}

	for _, output := range i.config.Output {
		i.output = append(i.output, output.converter)
	}

	return nil
}

func (i *Instance) Run() error {
	if len(i.input) == 0 || len(i.output) == 0 {
		return errors.New("input type and output type must be specified")
	}

	var err error
	container := NewContainer()
	for _, ic := range i.input {
		container, err = ic.Input(container)
		if err != nil {
			return err
		}
	}

	for _, oc := range i.output {
		if err := oc.Output(container); err != nil {
			return err
		}
	}

	return nil
}
