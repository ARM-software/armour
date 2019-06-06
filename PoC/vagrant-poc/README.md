**Vagrant setup**


1- Add [poc.box] (https://armh-my.sharepoint.com/:u:/g/personal/basma_elgaabouri_arm_com/EcZ9QKYGXQRFpeolPW-MIZsBFSztyHs4HS8Q-nPKlRG9kw?e=mvciHi) to the list of your local Vagrant boxes:

	vagrant box add poc.box --name poc
	vagrant up
	
2- Make sure vm is attached to a bridge adapter.
for login: 

	user: basma
	password: Basma.

	cd armour/PoC
	./clonePoC.sh
	cd ~/PoC
	docker-compose up
