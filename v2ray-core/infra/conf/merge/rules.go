// Copyright 2020 Jebbs. All rights reserved.
// Use of this source code is governed by MIT
// license that can be found in the LICENSE file.

package merge

import "fmt"

const (
	priorityKey string = "_priority"
	tagKey      string = "_tag"
)

// ApplyRules applies merge rules according to _tag, _priority fields, and remove them
func ApplyRules(m map[string]interface{}) error {
	err := sortMergeSlices(m, "")
	if err != nil {
		return err
	}
	removeHelperFields(m)
	return nil
}

// sortMergeSlices enumerates all slices in a map, to sort by priority and merge by tag
func sortMergeSlices(target map[string]interface{}, path string) error {
	for key, value := range target {
		if slice, ok := value.([]interface{}); ok {
			sortByPriority(slice)
			s, err := mergeSameTag(slice, fmt.Sprintf("%s.%s", path, key))
			if err != nil {
				return err
			}
			target[key] = s
			for _, item := range s {
				if m, ok := item.(map[string]interface{}); ok {
					sortMergeSlices(m, fmt.Sprintf("%s.%s[]", path, key))
				}
			}
		} else if field, ok := value.(map[string]interface{}); ok {
			sortMergeSlices(field, fmt.Sprintf("%s.%s", path, key))
		}
	}
	return nil
}

func removeHelperFields(target map[string]interface{}) {
	for key, value := range target {
		if key == priorityKey || key == tagKey {
			delete(target, key)
		} else if slice, ok := value.([]interface{}); ok {
			for _, e := range slice {
				if el, ok := e.(map[string]interface{}); ok {
					removeHelperFields(el)
				}
			}
		} else if field, ok := value.(map[string]interface{}); ok {
			removeHelperFields(field)
		}
	}
}
