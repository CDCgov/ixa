from cfa.cloudops import CloudClient

'''
This script needs the following in a file 'env'
AZURE_BATCH_ACCOUNT=
AZURE_BATCH_LOCATION=
AZURE_USER_ASSIGNED_IDENTITY=
AZURE_SUBNET_ID=
AZURE_CLIENT_ID=
AZURE_KEYVAULT_NAME=
AZURE_KEYVAULT_SP_SECRET_ID=

# Azure Blob storage config
AZURE_BLOB_STORAGE_ACCOUNT=

# Azure container registry config
AZURE_CONTAINER_REGISTRY_ACCOUNT=

# Azure SP info
AZURE_TENANT_ID=
AZURE_SUBSCRIPTION_ID=
AZURE_CLIENT_SECRET=
AZURE_RESOURCE_GROUP_NAME=
'''


DOCKER_IMAGE_NAME = "ixa-bench"
REGISTRY_NAME = "cfaprdbatchcr"
POOL_NAME = "ixa-pool1"
JOB_NAME = "ixa-job1"

def main():
    # initialize
    print("Initializing CloudClient...")
    cc = CloudClient(dotenv_path="env", use_sp = True)

    cc.upload_files(
        files = "ixa_setup.sh",
        container_name = "input-test",
        local_root_dir = "../",
        location_in_blob = "ixa-bench"
    )

    cc.create_pool(
        pool_name = POOL_NAME,
        mounts = ['input-test'],
        container_image_name = "rust:slim",
        vm_size = "standard_d8s_v3",
        max_autoscale_nodes = 1,
        autoscale = False
    )

    print("create_job")
    cc.create_job(
        JOB_NAME,
        pool_name = POOL_NAME,
        exist_ok = True
    )

    cc.add_task(
        job_name = JOB_NAME,
        command_line = "/input-test/ixa-bench/ixa_setup.sh"
    )

    cc.monitor_job(
        JOB_NAME
    )

    # get the stdout/stderr and print them?

    # delete the job
    cc.delete_job(JOB_NAME)

    # delete the pool
    cc.delete_pool(POOL_NAME)

if __name__ == "__main__":
    main()
