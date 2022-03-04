package libcore

import (
	"github.com/golang/protobuf/proto"
	"github.com/sirupsen/logrus"
	"github.com/v2fly/v2ray-core/v5/app/observatory"
	"github.com/v2fly/v2ray-core/v5/features/extension"
)

func (instance *V2RayInstance) GetObservatoryStatus(tag string) ([]byte, error) {
	if instance.observatory == nil {
		return nil, newError("observatory unavailable")
	}
	observer, err := instance.observatory.GetFeaturesByTag(tag)
	if err != nil {
		return nil, err
	}
	status, err := observer.(extension.Observatory).GetObservation(nil)
	if err != nil {
		return nil, err
	}
	return proto.Marshal(status)
}

func (instance *V2RayInstance) UpdateStatus(tag string, status []byte) error {
	if instance.observatory == nil {
		return newError("observatory unavailable")
	}

	s := new(observatory.OutboundStatus)
	err := proto.Unmarshal(status, s)
	if err != nil {
		return err
	}

	observer, err := instance.observatory.GetFeaturesByTag(tag)
	if err != nil {
		return err
	}
	observer.(*observatory.Observer).UpdateStatus(s)
	return err
}

type ObservatoryStatusUpdateListener interface {
	OnUpdateObservatoryStatus(status []byte) error
}

func (instance *V2RayInstance) SetStatusUpdateListener(tag string, listener ObservatoryStatusUpdateListener) error {
	if listener == nil {
		observer, err := instance.observatory.GetFeaturesByTag(tag)
		if err != nil {
			return err
		}
		observer.(*observatory.Observer).StatusUpdate = nil
	} else {
		observer, err := instance.observatory.GetFeaturesByTag(tag)
		if err != nil {
			return err
		}
		observer.(*observatory.Observer).StatusUpdate = func(result *observatory.OutboundStatus) {
			status, _ := proto.Marshal(result)
			err = listener.OnUpdateObservatoryStatus(status)
			if err != nil {
				logrus.Warn("failed to send observatory status update: ", err)
			}
		}
	}
	return nil
}
