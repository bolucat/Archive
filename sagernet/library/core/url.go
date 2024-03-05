package libcore

import (
	"errors"
	"net"
	"net/url"
	"strconv"
	"strings"
	_ "unsafe"
)

type URL interface {
	GetScheme() string
	SetScheme(scheme string)
	GetOpaque() string
	SetOpaque(opaque string)
	GetUsername() string
	SetUsername(username string)
	GetPassword() string
	SetPassword(password string) error
	GetHost() string
	SetHost(host string)
	GetPort() int32
	SetPort(port int32)
	GetPath() string
	SetPath(path string)
	GetRawPath() string
	SetRawPath(rawPath string) error
	QueryParameterNotBlank(key string) string
	AddQueryParameter(key, value string)
	GetFragment() string
	SetRawFragment(rawFragment string) error
	GetString() string
}

var _ URL = (*netURL)(nil)

type netURL struct {
	url.URL
	url.Values
}

func NewURL(scheme string) URL {
	u := new(netURL)
	u.Scheme = scheme
	u.Values = make(url.Values)
	return u
}

//go:linkname getScheme net/url.getScheme
func getScheme(rawURL string) (scheme, path string, err error)

//go:linkname parseAuthority net/url.parseAuthority
func parseAuthority(authority string) (user *url.Userinfo, host string, err error)

//go:linkname setFragment net/url.(*URL).setFragment
func setFragment(u *url.URL, fragment string) error

//go:linkname setPath net/url.(*URL).setPath
func setPath(u *url.URL, fragment string) error

// parse parses a URL from a string in one of two contexts. If
// viaRequest is true, the URL is assumed to have arrived via an HTTP request,
// in which case only absolute URLs or path-absolute relative URLs are allowed.
// If viaRequest is false, all forms of relative URLs are allowed.
func parse(rawURL string) (*url.URL, error) {
	var rest string
	var err error

	url := new(url.URL)

	if rawURL == "*" {
		url.Path = "*"
		return url, nil
	}

	// Split off possible leading "http:", "mailto:", etc.
	// Cannot contain escaped characters.
	if url.Scheme, rest, err = getScheme(rawURL); err != nil {
		return nil, err
	}
	url.Scheme = strings.ToLower(url.Scheme)

	if strings.HasSuffix(rest, "?") && strings.Count(rest, "?") == 1 {
		url.ForceQuery = true
		rest = rest[:len(rest)-1]
	} else {
		rest, url.RawQuery, _ = strings.Cut(rest, "?")
	}

	if !strings.HasPrefix(rest, "/") {
		if url.Scheme != "" {
			// We consider rootless paths per RFC 3986 as opaque.
			url.Opaque = rest
			return url, nil
		}

		// Avoid confusion with malformed schemes, like cache_object:foo/bar.
		// See golang.org/issue/16822.
		//
		// RFC 3986, §3.3:
		// In addition, a URI reference (Section 4.1) may be a relative-path reference,
		// in which case the first path segment cannot contain a colon (":") character.
		if segment, _, _ := strings.Cut(rest, "/"); strings.Contains(segment, ":") {
			// First path segment has colon. Not allowed in relative URL.
			return nil, errors.New("first path segment in URL cannot contain colon")
		}
	}

	if (url.Scheme != "" || !strings.HasPrefix(rest, "///")) && strings.HasPrefix(rest, "//") {
		var authority string
		authority, rest = rest[2:], ""
		if i := strings.Index(authority, "/"); i >= 0 {
			authority, rest = authority[:i], authority[i:]
		}
		url.User, url.Host, err = parseAuthority(authority)
		if err != nil {
			return nil, err
		}
	}
	// Set Path and, optionally, RawPath.
	// RawPath is a hint of the encoding of Path. We don't want to set it if
	// the default escaping of Path is equivalent, to help make sure that people
	// don't rely on it in general.
	if err := setPath(url, rest); err != nil {
		return nil, err
	}
	return url, nil
}

func ParseURL(rawURL string) (URL, error) {
	u := &netURL{}
	ru, frag, _ := strings.Cut(rawURL, "#")
	uu, err := parse(ru)
	if err != nil {
		return nil, newError("failed to parse url: ", rawURL).Base(err)
	}
	u.URL = *uu
	u.Values = u.Query()
	if u.Values == nil {
		u.Values = make(url.Values)
	}
	if frag == "" {
		return u, nil
	}
	if err = u.SetRawFragment(frag); err != nil {
		return nil, err
	}
	return u, nil
}

func (u *netURL) GetScheme() string {
	return u.Scheme
}

func (u *netURL) SetScheme(scheme string) {
	u.Scheme = scheme
}

func (u *netURL) GetOpaque() string {
	return u.Opaque
}

func (u *netURL) SetOpaque(opaque string) {
	u.Opaque = opaque
}

func (u *netURL) GetUsername() string {
	if u.User != nil {
		return u.User.Username()
	}
	return ""
}

func (u *netURL) SetUsername(username string) {
	if u.User != nil {
		if password, ok := u.User.Password(); !ok {
			u.User = url.User(username)
		} else {
			u.User = url.UserPassword(username, password)
		}
	} else {
		u.User = url.User(username)
	}
}

func (u *netURL) GetPassword() string {
	if u.User != nil {
		if password, ok := u.User.Password(); ok {
			return password
		}
	}
	return ""
}

func (u *netURL) SetPassword(password string) error {
	if u.User == nil {
		return newError("set username first")
	}
	u.User = url.UserPassword(u.User.Username(), password)
	return nil
}

func (u *netURL) GetHost() string {
	return u.Hostname()
}

func (u *netURL) SetHost(host string) {
	_, port, err := net.SplitHostPort(u.Host)
	if err == nil {
		u.Host = net.JoinHostPort(host, port)
	} else {
		u.Host = host
	}
}

func (u *netURL) GetPort() int32 {
	portStr := u.Port()
	if portStr == "" {
		return 0
	}
	port, _ := strconv.Atoi(portStr)
	return int32(port)
}

func (u *netURL) SetPort(port int32) {
	host, _, err := net.SplitHostPort(u.Host)
	if err == nil {
		u.Host = net.JoinHostPort(host, strconv.Itoa(int(port)))
	} else {
		u.Host = net.JoinHostPort(u.Host, strconv.Itoa(int(port)))
	}
}

func (u *netURL) GetPath() string {
	return u.Path
}

func (u *netURL) SetPath(path string) {
	u.Path = path
	u.RawPath = ""
}

func (u *netURL) GetRawPath() string {
	return u.RawPath
}

func (u *netURL) SetRawPath(rawPath string) error {
	return setPath(&u.URL, rawPath)
}

func (u *netURL) QueryParameterNotBlank(key string) string {
	return u.Get(key)
}

func (u *netURL) AddQueryParameter(key, value string) {
	u.Add(key, value)
}

func (u *netURL) GetFragment() string {
	return u.Fragment
}

func (u *netURL) SetRawFragment(rawFragment string) error {
	return setFragment(&u.URL, rawFragment)
}

func (u *netURL) GetString() string {
	u.RawQuery = u.Encode()
	return u.String()
}
