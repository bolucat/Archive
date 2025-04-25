package utils

import (
	"errors"
	"fmt"
	"reflect"
)

func Filter[T comparable](tSlice []T, filter func(t T) bool) []T {
	result := make([]T, 0)
	for _, t := range tSlice {
		if filter(t) {
			result = append(result, t)
		}
	}
	return result
}

func Map[T any, N any](arr []T, block func(it T) N) []N {
	if arr == nil { // keep nil
		return nil
	}
	retArr := make([]N, 0, len(arr))
	for index := range arr {
		retArr = append(retArr, block(arr[index]))
	}
	return retArr
}

func ToStringSlice(value any) ([]string, error) {
	strArr := make([]string, 0)
	switch reflect.TypeOf(value).Kind() {
	case reflect.Slice, reflect.Array:
		origin := reflect.ValueOf(value)
		for i := 0; i < origin.Len(); i++ {
			item := fmt.Sprintf("%v", origin.Index(i))
			strArr = append(strArr, item)
		}
	case reflect.String:
		strArr = append(strArr, fmt.Sprintf("%v", value))
	default:
		return nil, errors.New("value format error, must be string or array")
	}
	return strArr, nil
}
