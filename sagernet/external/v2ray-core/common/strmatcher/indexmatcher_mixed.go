package strmatcher

type MixedIndexMatcher struct {
	count  uint32
	mph    *MphMatcherGroup
	substr SubstrMatcherGroup
	regex  SimpleMatcherGroup
}

func NewMixedIndexMatcher() *MixedIndexMatcher {
	return new(MixedIndexMatcher)
}

// Add implements IndexMatcher.Add.
func (g *MixedIndexMatcher) Add(matcher Matcher) uint32 {
	g.count++
	index := g.count

	switch matcher := matcher.(type) {
	case FullMatcher:
		if g.mph == nil {
			g.mph = NewMphMatcherGroup()
		}
		g.mph.AddFullMatcher(matcher, index)
	case DomainMatcher:
		if g.mph == nil {
			g.mph = NewMphMatcherGroup()
		}
		g.mph.AddDomainMatcher(matcher, index)
	case SubstrMatcher:
		g.substr.AddSubstrMatcher(matcher, index)
	case *RegexMatcher:
		g.regex.AddMatcher(matcher, index)
	}

	return index
}

// Build implements IndexMatcher.Build.
func (g *MixedIndexMatcher) Build() error {
	if g.mph != nil {
		return g.mph.Build()
	}
	return nil
}

// Match implements IndexMatcher.Match.
func (g *MixedIndexMatcher) Match(input string) []uint32 {
	var result []uint32
	if g.mph != nil {
		result = append(result, g.mph.Match(input)...)
	}
	result = append(result, g.substr.Match(input)...)
	result = append(result, g.regex.Match(input)...)
	return result
}

// MatchAny implements IndexMatcher.MatchAny.
func (g *MixedIndexMatcher) MatchAny(input string) bool {
	if g.mph != nil && g.mph.MatchAny(input) {
		return true
	}
	return g.substr.MatchAny(input) || g.regex.MatchAny(input)
}

// Size implements IndexMatcher.Size.
func (g *MixedIndexMatcher) Size() uint32 {
	return g.count
}
