package libcore

import (
	"github.com/golang/protobuf/proto"
	"github.com/pkg/errors"
	"github.com/v2fly/v2ray-core/v4/app/observatory"
)

func (instance *V2RayInstance) GetObservatoryStatus() ([]byte, error) {
	if instance.observatory == nil {
		return nil, errors.New("observatory unavailable")
	}
	resp, err := instance.observatory.GetObservation(nil)
	if err != nil {
		return nil, err
	}
	return proto.Marshal(resp)
}

func (instance *V2RayInstance) UpdateStatus(outbound string, status []byte) error {
	if instance.observatory == nil {
		return errors.New("observatory unavailable")
	}
	s := new(observatory.OutboundStatus)
	err := proto.Unmarshal(status, s)
	if err != nil {
		return err
	}
	instance.observatory.UpdateStatus(outbound, s)
	return err
}
