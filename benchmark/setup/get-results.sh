aws ec2 describe-instances --filters "Name=image-id,Values=ami-0b97d0763dc595e32"  --region eu-west-2 --query 'Reservations[*].Instances[*].NetworkInterfaces[*].PrivateIpAddresses[*].[Association.PublicIp]' --output text > ips

ARRAY=( $( cat ips ) )
i=1
for ip in "${ARRAY[@]}"
do
  mkdir ../raw/$ip
  scp -i ~/Downloads/some-key.pem -o StrictHostKeyChecking=no -rp ec2-user@$ip:~/results/ ../raw/$ip
done
