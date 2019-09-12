Docker Engine Query Policy Service
==================================

## Requirements
- [Install capnproto](https://capnproto.org/install.html):
	- Debian / Ubuntu: 

			 apt-get install capnproto

	- Homebrew (OSX): 

			brew install capnp
			
- [Install pycapnp](http://capnproto.github.io/pycapnp/install.html):

		pip install -U cython
	 	pip install -U setuptools
		pip install pycapnp
- [Install Docker SDK for Python](https://docker-py.readthedocs.io/en/stable/):

		pip install docker
		
## Available policies
The current setup only allows to retrieve information about a docker container or image given an ID with a single or multiple hosts.

	fn info(ID, str, str) -> List<(str)>
	
- `ID` can be something like: `ID { hosts: {"alpine", "infallible_lewin"}, ips: {V4(1.10.0.3)}, port: Some(8080) }`
-  First `str` is either `container` or `image`
	- In case of `container`:
		Second `str` can be `id`, `image`, `labels`, `status`, `name`, `short_id`
	- In case of `image`: Second `str` can be `id`, `tag`, `labels`, `digest`
